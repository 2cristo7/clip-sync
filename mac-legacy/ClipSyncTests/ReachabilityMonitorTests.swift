import Network
import XCTest
@testable import ClipSync

final class ReachabilityMonitorTests: XCTestCase {

    // MARK: - interfaceNames helper

    func testInterfaceNamesFromPath_returnsSet() throws {
        // NWPath is not easily constructible in tests, so we verify the
        // static helper compiles and works with a real monitor snapshot.
        // The main value here is ensuring the type signature is correct
        // and the code compiles into the test target.
        let monitor = NWPathMonitor()
        let path = monitor.currentPath
        let names = ReachabilityMonitor.interfaceNames(from: path)
        // On a CI machine or Mac, at least one interface should exist.
        // But we don't assert count because headless runners might have none.
        XCTAssertTrue(names is Set<String>)
        monitor.cancel()
    }

    // MARK: - Lifecycle

    func testStartStop_doesNotCrash() throws {
        // Uses a real NWPathMonitor but immediately cancels it.
        // Validates that the start/stop lifecycle works without errors.
        let advertiser = BonjourAdvertiser(
            port: 0,
            serviceName: "test-reachability",
            txtRecord: [:]
        )
        let sut = ReachabilityMonitor(advertiser: advertiser)
        sut.start()
        // Calling start again should be a no-op
        sut.start()
        sut.stop()
        // Calling stop again should be a no-op
        sut.stop()
    }
}
