import Observation
import UIKit

/// Glue between the touch surface, the Rust remote and the drawing: every
/// touch goes to Rust synchronously, then the view redraws from its frame.
@MainActor
@Observable
final class N64Model {
    private(set) var layout: N64Layout?
    private(set) var frame: N64Frame?
    /// Buttons drawn as pressed: those held now, and those pressed less than
    /// `minimumPress` ago. A tap can go down and up before the next frame is
    /// drawn, and would otherwise never show.
    private(set) var shown: Set<N64Button> = []

    /// Only the drawing waits: the pad itself already reads the release.
    static let minimumPress = Duration.milliseconds(60)

    @ObservationIgnored private var remote: N64Remote?
    @ObservationIgnored private let haptics = Haptics()
    @ObservationIgnored private var pressedAt: [N64Button: ContinuousClock.Instant] = [:]

    func handle(_ event: TouchSurface.Event) {
        switch event {
        case let .layout(size, insets):
            let screen = Size(width: Float(size.width), height: Float(size.height))
            let safe = Insets(
                top: Float(insets.top), left: Float(insets.left),
                bottom: Float(insets.bottom), right: Float(insets.right))
            if let remote {
                remote.resize(screen: screen, insets: safe)
            } else {
                remote = N64Remote(screen: screen, insets: safe)
            }
            layout = remote?.layout()
        case let .down(id, at):
            haptics.prepare()
            haptics.play(remote?.touchDown(id: id, point: Point(at)))
        case let .move(id, at):
            haptics.play(remote?.touchMove(id: id, point: Point(at)))
        case let .up(id):
            haptics.play(remote?.touchUp(id: id))
        }
        update()
    }

    /// The app left the foreground: nothing may stay held.
    func releaseAll() {
        remote?.releaseAll()
        update()
    }

    func isPressed(_ button: N64Button) -> Bool {
        shown.contains(button)
    }

    private func update() {
        let before = Set(frame?.pressed ?? [])
        frame = remote?.frame()
        let held = Set(frame?.pressed ?? [])
        let now = ContinuousClock.now
        for button in held.subtracting(before) {
            pressedAt[button] = now
        }
        refreshShown(now: now)
    }

    private func refreshShown(now: ContinuousClock.Instant) {
        let held = Set(frame?.pressed ?? [])
        pressedAt = pressedAt.filter { held.contains($0.key) || now - $0.value < Self.minimumPress }
        let lingering = Set(pressedAt.keys).subtracting(held)
        let next = held.union(lingering)
        if next != shown { shown = next }
        guard let oldest = lingering.compactMap({ pressedAt[$0] }).min() else { return }
        Task { [weak self] in
            try? await Task.sleep(until: oldest + Self.minimumPress, clock: .continuous)
            self?.refreshShown(now: .now)
        }
    }
}

extension Point {
    init(_ point: CGPoint) {
        self.init(x: Float(point.x), y: Float(point.y))
    }

    var cgPoint: CGPoint { CGPoint(x: CGFloat(x), y: CGFloat(y)) }
}

/// A light tap on press and a selection tick on release, which players of
/// touch emulators found clearer than a medium impact (Delta #57, MAME4iOS).
/// Generators stay prepared for a few seconds only, and firing one ends it:
/// prepare when a finger lands and again after each haptic (Apple's
/// `UIFeedbackGenerator.prepare()` notes).
@MainActor
final class Haptics {
    private let press = UIImpactFeedbackGenerator(style: .light)
    private let release = UISelectionFeedbackGenerator()
    private let edge = UIImpactFeedbackGenerator(style: .rigid)

    init() {
        prepare()
    }

    func prepare() {
        press.prepare()
        release.prepare()
        edge.prepare()
    }

    func play(_ haptic: Haptic?) {
        switch haptic {
        case .press:
            press.impactOccurred()
            press.prepare()
        case .release:
            release.selectionChanged()
            release.prepare()
        case .edge:
            edge.impactOccurred(intensity: 0.5)
            edge.prepare()
        case nil:
            break
        }
    }
}
