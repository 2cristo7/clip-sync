import Foundation
import Crypto
import X509
import SwiftASN1
import NIOSSL
import Logging

enum TLSManagerError: Error {
    case serializationFailed
    case storageFailed
}

/// Generates and persists a self-signed TLS identity (EC P-256) for the ClipSync server.
///
/// On first launch, generates an EC P-256 key + self-signed certificate covering
/// `localhost`, `<hostname>.local` and the host's primary IPv4 address, and stores
/// both the DER-encoded cert and the PEM-encoded key in the Keychain.
///
/// On subsequent launches, the persisted identity is loaded and reused so the SPKI
/// fingerprint (and therefore cert pinning) remains stable across restarts.
final class TLSManager: @unchecked Sendable {
    private let keychain: Keychain
    private var logger: Logger

    private let certAccount = "tls-cert-der"
    private let keyAccount = "tls-key-pem"

    private(set) var certificateDER: Data = Data()
    private(set) var privateKeyPEM: String = ""

    /// SPKI-SHA256 base64url (no padding) fingerprint, suitable for cert pinning
    /// and for advertising via Bonjour TXT `fp`.
    private(set) var spkiFingerprint: String = ""

    init(keychain: Keychain = Keychain(service: TLSManager.service),
         logger: Logger = Logger(label: "clipsync.tls")) {
        self.keychain = keychain
        self.logger = logger
    }

    static let service = "com.clipsync.tls-identity"

    /// Loads the persisted identity, or creates a new one and persists it.
    func loadOrCreate() throws {
        if let existing = try loadExisting() {
            self.certificateDER = existing.certDER
            self.privateKeyPEM = existing.keyPEM
            self.spkiFingerprint = try Self.spkiFingerprint(certDER: existing.certDER)
            logger.info("TLS identity loaded from keychain", metadata: [
                "fp": .string(spkiFingerprint),
            ])
            return
        }

        let identity = try Self.generateSelfSigned(
            hostnames: Self.defaultSANHostnames(),
            ipAddresses: Self.defaultSANIPv4()
        )
        try keychain.save(identity.certDER, account: certAccount)
        try keychain.save(Data(identity.keyPEM.utf8), account: keyAccount)

        self.certificateDER = identity.certDER
        self.privateKeyPEM = identity.keyPEM
        self.spkiFingerprint = try Self.spkiFingerprint(certDER: identity.certDER)
        logger.info("TLS identity generated and persisted", metadata: [
            "fp": .string(spkiFingerprint),
        ])
    }

    /// Builds a server-side NIOSSL TLSConfiguration backed by this identity.
    func makeServerTLSConfiguration() throws -> TLSConfiguration {
        let cert = try NIOSSLCertificate(bytes: [UInt8](certificateDER), format: .der)
        let key = try NIOSSLPrivateKey(bytes: [UInt8](privateKeyPEM.utf8), format: .pem)
        return TLSConfiguration.makeServerConfiguration(
            certificateChain: [.certificate(cert)],
            privateKey: .privateKey(key)
        )
    }

    private func loadExisting() throws -> (certDER: Data, keyPEM: String)? {
        do {
            let certDER = try keychain.load(account: certAccount)
            let keyData = try keychain.load(account: keyAccount)
            guard let keyPEM = String(data: keyData, encoding: .utf8) else {
                return nil
            }
            return (certDER, keyPEM)
        } catch KeychainError.notFound {
            return nil
        }
    }

    // MARK: - Cert generation

    struct GeneratedIdentity {
        let certDER: Data
        let keyPEM: String
    }

    /// Generates an EC P-256 self-signed certificate with the given SAN entries.
    static func generateSelfSigned(hostnames: [String],
                                   ipAddresses: [String],
                                   notValidBefore: Date = Date().addingTimeInterval(-300),
                                   notValidAfter: Date = Date().addingTimeInterval(60 * 60 * 24 * 365 * 5)) throws -> GeneratedIdentity {
        let swiftCryptoKey = P256.Signing.PrivateKey()
        let certPrivateKey = Certificate.PrivateKey(swiftCryptoKey)

        let name = try DistinguishedName {
            CommonName("ClipSync")
            OrganizationName("ClipSync")
        }

        var sanNames: [GeneralName] = []
        for host in hostnames {
            sanNames.append(.dnsName(host))
        }
        for ip in ipAddresses {
            // IPv4 as 4 raw bytes per RFC 5280.
            if let bytes = ipv4RawBytes(ip) {
                sanNames.append(.ipAddress(ASN1OctetString(contentBytes: ArraySlice(bytes))))
            }
        }

        let extensions = try Certificate.Extensions {
            Critical(
                BasicConstraints.notCertificateAuthority
            )
            Critical(
                KeyUsage(digitalSignature: true, keyEncipherment: true)
            )
            try ExtendedKeyUsage([.serverAuth, .clientAuth])
            SubjectAlternativeNames(sanNames)
            SubjectKeyIdentifier(keyIdentifier: ArraySlice(subjectKeyIdentifier(for: certPrivateKey.publicKey)))
        }

        let serial: Certificate.SerialNumber = {
            var bytes = [UInt8](repeating: 0, count: 16)
            _ = bytes.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!) }
            // Ensure positive serial
            bytes[0] &= 0x7F
            if bytes[0] == 0 { bytes[0] = 0x01 }
            return .init(bytes: ArraySlice(bytes))
        }()

        let certificate = try Certificate(
            version: .v3,
            serialNumber: serial,
            publicKey: certPrivateKey.publicKey,
            notValidBefore: notValidBefore,
            notValidAfter: notValidAfter,
            issuer: name,
            subject: name,
            extensions: extensions,
            issuerPrivateKey: certPrivateKey
        )

        var serializer = DER.Serializer()
        try serializer.serialize(certificate)
        let certDER = Data(serializer.serializedBytes)

        let keyPEM = swiftCryptoKey.pemRepresentation
        return GeneratedIdentity(certDER: certDER, keyPEM: keyPEM)
    }

    /// Computes SPKI SHA-256 base64url-without-padding from a DER-encoded certificate.
    static func spkiFingerprint(certDER: Data) throws -> String {
        let cert = try Certificate(derEncoded: [UInt8](certDER))
        var serializer = DER.Serializer()
        try serializer.serialize(cert.publicKey)
        let spkiBytes = serializer.serializedBytes
        let digest = SHA256.hash(data: spkiBytes)
        return base64URLNoPadding(Data(digest))
    }

    static func base64URLNoPadding(_ data: Data) -> String {
        let base64 = data.base64EncodedString()
        return base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    // MARK: - SAN helpers

    static func defaultSANHostnames() -> [String] {
        var hosts: Set<String> = ["localhost"]
        let host = ProcessInfo.processInfo.hostName
        if !host.isEmpty { hosts.insert(host) }
        if let base = host.components(separatedBy: ".").first, !base.isEmpty {
            hosts.insert("\(base).local")
        }
        return Array(hosts)
    }

    static func defaultSANIPv4() -> [String] {
        var addrs: [String] = ["127.0.0.1"]
        if let primary = primaryIPv4Address() {
            addrs.append(primary)
        }
        return addrs
    }

    static func primaryIPv4Address() -> String? {
        var ifaddr: UnsafeMutablePointer<ifaddrs>? = nil
        guard getifaddrs(&ifaddr) == 0, let first = ifaddr else { return nil }
        defer { freeifaddrs(ifaddr) }

        var result: String?
        var ptr: UnsafeMutablePointer<ifaddrs>? = first
        while let current = ptr {
            let flags = Int32(current.pointee.ifa_flags)
            let addr = current.pointee.ifa_addr
            if let addr = addr, addr.pointee.sa_family == UInt8(AF_INET),
               (flags & IFF_UP) != 0,
               (flags & IFF_LOOPBACK) == 0 {
                var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
                if getnameinfo(addr,
                               socklen_t(addr.pointee.sa_len),
                               &host, socklen_t(host.count),
                               nil, 0,
                               NI_NUMERICHOST) == 0 {
                    let s = String(cString: host)
                    if s != "127.0.0.1" {
                        result = s
                        break
                    }
                }
            }
            ptr = current.pointee.ifa_next
        }
        return result
    }
}

private func ipv4RawBytes(_ ip: String) -> [UInt8]? {
    var addr = in_addr()
    guard inet_pton(AF_INET, ip, &addr) == 1 else { return nil }
    var copy = addr.s_addr
    return withUnsafeBytes(of: &copy) { Array($0) }
}

private func subjectKeyIdentifier(for publicKey: Certificate.PublicKey) -> [UInt8] {
    // Per RFC 5280 §4.2.1.2 method (1): SHA-1 of the BIT STRING subjectPublicKey (no tag/length).
    // For simplicity and non-critical use, we compute SHA-256 of the SPKI and take the leading 20 bytes.
    var serializer = DER.Serializer()
    do {
        try serializer.serialize(publicKey)
    } catch {
        return [UInt8](repeating: 0, count: 20)
    }
    let digest = SHA256.hash(data: serializer.serializedBytes)
    return Array(digest.prefix(20))
}
