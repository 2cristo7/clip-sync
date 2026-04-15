import XCTest
import Hummingbird
@testable import ClipSync

final class AuthMiddlewareTests: XCTestCase {
    func testExtractBearer() {
        XCTAssertEqual(
            AuthMiddleware<BasicRequestContext>.extractBearer("Bearer abc123"),
            "abc123"
        )
        XCTAssertEqual(
            AuthMiddleware<BasicRequestContext>.extractBearer("bearer   x"),
            "x"
        )
        XCTAssertNil(AuthMiddleware<BasicRequestContext>.extractBearer(nil))
        XCTAssertNil(AuthMiddleware<BasicRequestContext>.extractBearer(""))
        XCTAssertNil(AuthMiddleware<BasicRequestContext>.extractBearer("Basic Zm9vOmJhcg=="))
        XCTAssertNil(AuthMiddleware<BasicRequestContext>.extractBearer("Bearer "))
    }

    // Integration behaviour (bearer + HMAC both required for /inject, revoked
    // tokens get 401, etc.) is verified indirectly through HMACValidatorTests
    // and TokenStoreTests; a full Hummingbird integration harness is intentionally
    // not pulled in to avoid framework-linking issues on macOS 14.
}
