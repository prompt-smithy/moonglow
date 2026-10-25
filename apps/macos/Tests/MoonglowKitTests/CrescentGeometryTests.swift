import Foundation
import Testing
@testable import MoonglowKit

/// Guards the shared crescent outline: one continuous, non-self-intersecting loop
/// that encloses a crescent opening towards the upper right, matching the
/// `tools/generate-logo-assets.ps1` master.
@Suite("Crescent geometry")
struct CrescentGeometryTests {

    @Test("Constants match the shared artwork master")
    func constantsMatchMaster() {
        #expect(CrescentGeometry.design == 1024)
        #expect(CrescentGeometry.crescentRadius == 480)
        #expect(CrescentGeometry.cutRadiusFactor == 1.05)
        #expect(CrescentGeometry.crescentThickness == 0.6)
        #expect(CrescentGeometry.keylineDesign == 24)
    }

    @Test("Outline is one continuous loop with no self-intersections")
    func outlineIsSimpleLoop() {
        let outline = CrescentGeometry.outline()
        #expect(outline.count > 100)
        // One-degree sampling on a radius-480 rim steps ~8.8 units at most.
        #expect(maxStep(outline) < 12)
        #expect(isSimple(outline))
    }

    @Test("Walking the inner rim forward folds the loop into a bowtie")
    func innerRimMustBeWalkedBack() {
        // Negative control for the ordering the status icon shipped with: the same two
        // rims composed without reversing the inner one. It must trip every detector
        // above, proving they catch a real defect rather than passing vacuously.
        var broken = CrescentGeometry.outerArc()
        broken.append(contentsOf: CrescentGeometry.innerArc().dropFirst().dropLast())
        #expect(maxStep(broken) > 100)
        #expect(!isSimple(broken))
    }

    @Test("Outline encloses a crescent, not a disc")
    func outlineIsACrescent() {
        let outline = CrescentGeometry.outline()
        let disc = Double.pi * CrescentGeometry.crescentRadius * CrescentGeometry.crescentRadius
        // The true mark covers ~37% of the disc; the bowtie ordering covers ~93%.
        let fraction = abs(shoelaceArea(outline)) / disc
        #expect(fraction > 0.25 && fraction < 0.55)
        // The bulk of the mark sits opposite the opening (upper right), so the area
        // centroid is well down-left of the design centre (512, 512).
        let centroid = areaCentroid(outline)
        #expect(centroid.x < 480 && centroid.y < 480)
    }
}

// MARK: - Polygon predicates (test-only)

private typealias Point = CrescentGeometry.Point

/// Longest step between consecutive vertices, closing the loop.
private func maxStep(_ points: [Point]) -> Double {
    var longest = 0.0
    for index in points.indices {
        let from = points[index]
        let to = points[(index + 1) % points.count]
        longest = max(longest, hypot(to.x - from.x, to.y - from.y))
    }
    return longest
}

/// True when no two non-adjacent edges properly cross.
private func isSimple(_ points: [Point]) -> Bool {
    let count = points.count
    for i in 0..<count {
        let a1 = points[i]
        let a2 = points[(i + 1) % count]
        for j in (i + 1)..<count {
            // Segments sharing a vertex are neighbours, not crossings.
            if j == i + 1 || (i == 0 && j == count - 1) { continue }
            if segmentsCross(a1, a2, points[j], points[(j + 1) % count]) { return false }
        }
    }
    return true
}

private func segmentsCross(_ a: Point, _ b: Point, _ c: Point, _ d: Point) -> Bool {
    let o1 = orient(a, b, c)
    let o2 = orient(a, b, d)
    let o3 = orient(c, d, a)
    let o4 = orient(c, d, b)
    return ((o1 > 0) != (o2 > 0)) && ((o3 > 0) != (o4 > 0))
}

private func orient(_ a: Point, _ b: Point, _ c: Point) -> Double {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

private func shoelaceArea(_ points: [Point]) -> Double {
    var sum = 0.0
    for index in points.indices {
        let from = points[index]
        let to = points[(index + 1) % points.count]
        sum += from.x * to.y - to.x * from.y
    }
    return sum / 2
}

private func areaCentroid(_ points: [Point]) -> Point {
    let area = shoelaceArea(points)
    var x = 0.0
    var y = 0.0
    for index in points.indices {
        let from = points[index]
        let to = points[(index + 1) % points.count]
        let cross = from.x * to.y - to.x * from.y
        x += (from.x + to.x) * cross
        y += (from.y + to.y) * cross
    }
    return Point(x: x / (6 * area), y: y / (6 * area))
}
