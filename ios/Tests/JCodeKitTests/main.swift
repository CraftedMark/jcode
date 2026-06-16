import XCTest
@testable import JCodeKit

final class JCodeKitHarnessTests: XCTestCase {
    override func setUp() {
        super.setUp()
        passed = 0
        failed = 0
        passed2 = 0
        failed2 = 0
    }

    func testProtocolHarnessAssertions() {
        runProtocolTests()

        XCTAssertEqual(failed, 0)
        XCTAssertGreaterThan(passed, 0)
    }

    func testClientHarnessAssertions() {
        runClientTests()

        XCTAssertEqual(failed2, 0)
        XCTAssertGreaterThan(passed2, 0)
    }
}
