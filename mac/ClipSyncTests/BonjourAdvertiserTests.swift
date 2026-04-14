import XCTest
@testable import ClipSync

final class BonjourAdvertiserTests: XCTestCase {
    func testAdvertiserStartsAndStopsWithoutCrashing() {
        let advertiser = BonjourAdvertiser(
            serviceType: "_clipsync-test._tcp",
            port: 17010,
            serviceName: "ClipSyncTest-\(UUID().uuidString.prefix(6))",
            txtRecord: ["version": "0.1.0", "fp": "deadbeef"]
        )
        advertiser.start()
        advertiser.stop()
    }
}
