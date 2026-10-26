import CoreMotion
import Observation
import QuartzCore
import UIKit

/// The pad the game shows: every touch and motion sample goes to the Rust
/// session synchronously, then the view redraws from its frame.
@MainActor
@Observable
final class RemoteModel {
    private(set) var layout: RemoteLayout?
    private(set) var frame: Frame?
    /// Buttons drawn as pressed: those held now, and those pressed less than
    /// `minimumPress` ago. A tap can go down and up before the next frame is
    /// drawn, and would otherwise never show.
    private(set) var shown: Set<UInt8> = []
    /// Finished lines of a canvas, fetched only when they change.
    private(set) var strokes: [Stroke] = []

    /// Only the drawing waits: the game already has the release.
    static let minimumPress = Duration.milliseconds(60)

    @ObservationIgnored private let session: Session
    @ObservationIgnored private let haptics = Haptics()
    @ObservationIgnored private var pressedAt: [UInt8: ContinuousClock.Instant] = [:]
    @ObservationIgnored private var landscapeLeft = true
    @ObservationIgnored private var motion: MotionReader?

    init(session: Session) {
        self.session = session
    }

    /// The game sent a new layout.
    func reload() {
        layout = session.layout()
        update()
        let needsMotion = layout?.tilt == true || layout?.pointer == true
        if needsMotion, motion == nil {
            motion = MotionReader { [weak self] sample in self?.motion(sample) }
        } else if !needsMotion {
            motion?.stop()
            motion = nil
        }
    }

    func handle(_ event: TouchSurface.Event) {
        switch event {
        case let .layout(size, insets, left):
            landscapeLeft = left
            session.resize(
                screen: Size(width: Float(size.width), height: Float(size.height)),
                insets: Insets(
                    top: Float(insets.top), left: Float(insets.left),
                    bottom: Float(insets.bottom), right: Float(insets.right)))
            layout = session.layout()
        case let .down(id, at, time):
            haptics.prepare()
            haptics.play(session.touchDown(id: id, point: Point(at), time: time))
        case let .move(id, at, time):
            haptics.play(session.touchMove(id: id, point: Point(at), time: time))
        case let .up(id):
            haptics.play(session.touchUp(id: id))
        }
        update()
    }

    func stop() {
        motion?.stop()
        motion = nil
        releaseAll()
    }

    /// The app left the foreground or the connection dropped: nothing may
    /// stay held.
    func releaseAll() {
        session.releaseAll()
        update()
    }

    func isPressed(_ id: UInt8) -> Bool {
        shown.contains(id)
    }

    func isSelected(_ id: UInt8) -> Bool {
        frame?.selected.contains(id) ?? false
    }

    func knob(_ id: UInt8) -> Point? {
        frame?.knobs.first { $0.id == id }?.center
    }

    private func motion(_ sample: CMDeviceMotion) {
        session.motion(motion: Motion(
            gravityX: Float(sample.gravity.x), gravityY: Float(sample.gravity.y), gravityZ: Float(sample.gravity.z),
            rotationX: Float(sample.rotationRate.x), rotationY: Float(sample.rotationRate.y),
            rotationZ: Float(sample.rotationRate.z),
            time: sample.timestamp, landscapeLeft: landscapeLeft))
        update()
    }

    private func update() {
        let before = Set(frame?.pressed ?? Data())
        let revision = frame?.drawingRevision
        frame = session.frame(time: ProcessInfo.processInfo.systemUptime)
        if frame?.drawingRevision != revision {
            strokes = session.strokes()
        }
        let held = Set(frame?.pressed ?? Data())
        let now = ContinuousClock.now
        for id in held.subtracting(before) {
            pressedAt[id] = now
        }
        refreshShown(now: now)
    }

    private func refreshShown(now: ContinuousClock.Instant) {
        let held = Set(frame?.pressed ?? Data())
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

extension Rect {
    var cgRect: CGRect {
        CGRect(x: CGFloat(left), y: CGFloat(top), width: CGFloat(right - left), height: CGFloat(bottom - top))
    }
}

/// Reads the fused device motion once per screen refresh, the way Apple
/// recommends for games. The Simulator has no motion: nothing is read there.
@MainActor
final class MotionReader {
    private let manager = CMMotionManager()
    private var ticker: Ticker?
    private let onSample: (CMDeviceMotion) -> Void
    private var lastTimestamp: TimeInterval = 0

    init(onSample: @escaping (CMDeviceMotion) -> Void) {
        self.onSample = onSample
        guard manager.isDeviceMotionAvailable else { return }
        manager.deviceMotionUpdateInterval = 1.0 / 100.0
        manager.startDeviceMotionUpdates()
        ticker = Ticker { [weak self] in self?.tick() }
    }

    /// Runs until stopped, whoever still holds the reader.
    func stop() {
        ticker?.stop()
        ticker = nil
        manager.stopDeviceMotionUpdates()
    }

    private func tick() {
        guard let sample = manager.deviceMotion, sample.timestamp != lastTimestamp else { return }
        lastTimestamp = sample.timestamp
        onSample(sample)
    }
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
    private let notification = UINotificationFeedbackGenerator()

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
        case .success:
            notification.notificationOccurred(.success)
        case .warning:
            notification.notificationOccurred(.warning)
        case nil:
            break
        }
    }
}
