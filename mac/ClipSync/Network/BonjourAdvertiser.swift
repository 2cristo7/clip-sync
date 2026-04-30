import Foundation
import Logging

final class BonjourAdvertiser: NSObject {
    private let type: String
    private let port: Int32
    private let serviceName: String
    private let txtRecord: [String: String]
    private var logger: Logger
    private var service: NetService?

    /// `true` once `netServiceDidPublish` fires; reset to `false` on failure or stop.
    private(set) var isPublished = false

    /// Called on the main thread when mDNS publishing fails.
    /// The `Error` argument is an `NSError` constructed from the NetService error dict.
    var onPublishFailed: ((Error) -> Void)?

    init(serviceType: String = "_clipsync._tcp",
         port: Int32,
         serviceName: String,
         txtRecord: [String: String],
         logger: Logger = Logger(label: "clipsync.bonjour")) {
        self.type = serviceType
        self.port = port
        self.serviceName = serviceName
        self.txtRecord = txtRecord
        self.logger = logger
    }

    func start() {
        let perform = { [self] in
            guard service == nil else { return }
            let svc = NetService(
                domain: "",
                type: type,
                name: serviceName,
                port: port
            )
            svc.delegate = self
            svc.includesPeerToPeer = true
            svc.schedule(in: .main, forMode: .common)
            var dict: [String: Data] = [:]
            for (key, value) in txtRecord {
                dict[key] = Data(value.utf8)
            }
            svc.setTXTRecord(NetService.data(fromTXTRecord: dict))
            svc.publish()
            service = svc
            logger.info("mDNS publishing", metadata: [
                "type": .string(type),
                "name": .string(serviceName),
                "port": .stringConvertible(port),
            ])
        }
        if Thread.isMainThread {
            perform()
        } else {
            DispatchQueue.main.async(execute: perform)
        }
    }

    func stop() {
        let perform = { [self] in
            service?.stop()
            service = nil
        }
        if Thread.isMainThread {
            perform()
        } else {
            DispatchQueue.main.async(execute: perform)
        }
    }
}

extension BonjourAdvertiser: NetServiceDelegate {
    func netServiceDidPublish(_ sender: NetService) {
        isPublished = true
        logger.info("mDNS published", metadata: [
            "name": .string(sender.name),
            "port": .stringConvertible(sender.port),
        ])
    }

    func netService(_ sender: NetService, didNotPublish errorDict: [String: NSNumber]) {
        isPublished = false
        let code = errorDict[NetService.errorCode]?.intValue ?? -1
        let error = NSError(
            domain: NetService.errorDomain,
            code: code,
            userInfo: [NSLocalizedDescriptionKey: "mDNS publish failed (code \(code)) for '\(sender.name)'"]
        )
        logger.error("mDNS publish failed", metadata: [
            "name": .string(sender.name),
            "error": .string(String(describing: errorDict)),
        ])
        onPublishFailed?(error)
    }
}
