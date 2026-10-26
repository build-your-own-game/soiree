import SwiftUI

/// The remote the game composed: everything the player touches goes to Rust
/// through the touch surface on top; this view only draws.
struct PadScreen: View {
    let model: RemoteModel

    var body: some View {
        ZStack {
            if let layout = model.layout {
                LayoutBackdrop(model: model, layout: layout)
                ForEach(Array(layout.dpads.enumerated()), id: \.offset) { _, dpad in
                    DPadView(dpad: dpad, pressed: model.shown)
                }
                ForEach(layout.sticks, id: \.id) { stick in
                    StickView(stick: stick, knob: model.knob(stick.id))
                }
                ForEach(layout.controls, id: \.id) { control in
                    ControlView(
                        control: control,
                        pressed: model.isPressed(control.id),
                        selected: model.isSelected(control.id),
                        dot: control.dot.map { CGFloat($0) })
                }
                WireReadout(bytes: model.frame?.wire)
                    .position(x: CGFloat(layout.area.left + layout.area.right) / 2, y: CGFloat(layout.area.bottom) - 2)
            }
            TouchSurface { model.handle($0) }
        }
    }
}

/// What the game receives: proof that Rust sees what the fingers do.
private struct WireReadout: View {
    let bytes: Data?

    var body: some View {
        Text((bytes.map(Array.init) ?? []).map { String(format: "%02X", $0) }.joined(separator: " "))
            .font(.system(size: 10, weight: .semibold, design: .monospaced))
            .foregroundStyle(.white.opacity(0.35))
            .allowsHitTesting(false)
    }
}
