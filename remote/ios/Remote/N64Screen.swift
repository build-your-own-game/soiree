import SwiftUI

/// The Nintendo 64 remote, full screen. Rust decides where everything is and
/// what is pressed; this file only draws it.
struct N64Screen: View {
    @State private var model = N64Model()
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        ZStack {
            N64Palette.body.overlay(
                RadialGradient(
                    colors: [N64Palette.bodyLight, N64Palette.body],
                    center: .top, startRadius: 0, endRadius: 700))
            if let layout = model.layout {
                N64PadView(layout: layout, model: model)
            }
            TouchSurface { model.handle($0) }
        }
        .ignoresSafeArea()
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .defersSystemGestures(on: .all)
        .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { model.releaseAll() }
        }
    }
}

enum N64Palette {
    static let body = Color(red: 0.13, green: 0.135, blue: 0.15)
    static let bodyLight = Color(red: 0.22, green: 0.225, blue: 0.245)
    static let grip = Color(red: 0.19, green: 0.195, blue: 0.215)
    static let shell = Color(red: 0.55, green: 0.56, blue: 0.58)
    static let a = Color(red: 0.16, green: 0.36, blue: 0.84)
    static let b = Color(red: 0.13, green: 0.6, blue: 0.3)
    static let c = Color(red: 0.98, green: 0.78, blue: 0.1)
    static let cArrow = Color(red: 0.42, green: 0.31, blue: 0.0)
    static let start = Color(red: 0.86, green: 0.19, blue: 0.16)
    static let dpad = Color(red: 0.36, green: 0.37, blue: 0.4)
    static let gate = Color(red: 0.07, green: 0.075, blue: 0.085)
    static let knob = Color(red: 0.78, green: 0.79, blue: 0.81)
}

private struct N64PadView: View {
    let layout: N64Layout
    let model: N64Model

    var body: some View {
        let h = CGFloat(layout.area.bottom - layout.area.top)
        ZStack {
            grips(h: h)
            DPadView(layout: layout.dpad, model: model)
            StickView(layout: layout.stick, frame: model.frame)
            ForEach(layout.buttons, id: \.button) { button in
                ButtonView(layout: button, pressed: model.isPressed(button.button))
            }
            InputReadout(frame: model.frame)
                .position(x: CGFloat(layout.area.left + layout.area.right) / 2, y: CGFloat(layout.area.top) + 82)
        }
    }

    /// Two soft plates under the thumbs, a nod to the controller's prongs.
    private func grips(h: CGFloat) -> some View {
        let right = layout.buttons.first { $0.button == .a }?.center.cgPoint ?? .zero
        let cUp = layout.buttons.first { $0.button == .cUp }?.center.cgPoint ?? .zero
        let left = CGPoint(
            x: (layout.dpad.center.cgPoint.x + layout.stick.center.cgPoint.x) / 2,
            y: (layout.dpad.center.cgPoint.y + layout.stick.center.cgPoint.y) / 2)
        let rightCenter = CGPoint(x: (right.x + cUp.x) / 2 + 0.05 * h, y: (right.y + cUp.y) / 2 + 0.05 * h)
        return ZStack {
            grip(diameter: 0.98 * h).position(left)
            grip(diameter: 0.98 * h).position(rightCenter)
        }
    }

    private func grip(diameter: CGFloat) -> some View {
        Circle()
            .fill(RadialGradient(
                colors: [N64Palette.grip, N64Palette.body.opacity(0)],
                center: .center, startRadius: diameter * 0.3, endRadius: diameter / 2))
            .frame(width: diameter, height: diameter)
    }
}

private struct ButtonView: View {
    let layout: ButtonLayout
    let pressed: Bool

    var body: some View {
        ZStack {
            // A pressed button is hidden under the thumb: a halo wider than
            // the button shows around it.
            halo.opacity(pressed ? 1 : 0)
            shape
                .scaleEffect(pressed ? 0.94 : 1)
                .brightness(pressed ? -0.14 : 0)
        }
        .position(layout.center.cgPoint)
    }

    @ViewBuilder private var halo: some View {
        switch layout.shape {
        case let .circle(radius):
            Circle()
                .fill(RadialGradient(
                    colors: [color.opacity(0.55), color.opacity(0)],
                    center: .center, startRadius: CGFloat(radius) * 0.9, endRadius: CGFloat(radius) * 1.6))
                .frame(width: CGFloat(radius) * 3.2, height: CGFloat(radius) * 3.2)
        case let .capsule(width, height):
            Capsule()
                .fill(.white.opacity(0.18))
                .frame(width: CGFloat(width) + 24, height: CGFloat(height) + 24)
                .blur(radius: 8)
        }
    }

    @ViewBuilder private var shape: some View {
        switch layout.shape {
        case let .circle(radius):
            let r = CGFloat(radius)
            Circle()
                .fill(RadialGradient(
                    colors: [color.mix(with: .white, by: 0.3), color, color.mix(with: .black, by: 0.25)],
                    center: UnitPoint(x: 0.38, y: 0.3), startRadius: 0, endRadius: r * 2))
                .overlay(Circle().strokeBorder(.black.opacity(0.4), lineWidth: max(1.5, r * 0.07)))
                .overlay { label(size: r) }
                .frame(width: r * 2, height: r * 2)
                .shadow(color: .black.opacity(pressed ? 0.25 : 0.55), radius: pressed ? 1 : r * 0.14, y: pressed ? 1 : r * 0.12)
        case let .capsule(width, height):
            let (w, hh) = (CGFloat(width), CGFloat(height))
            Capsule()
                .fill(LinearGradient(
                    colors: [N64Palette.shell.mix(with: .white, by: 0.2), N64Palette.shell.mix(with: .black, by: 0.2)],
                    startPoint: .top, endPoint: .bottom))
                .overlay(Capsule().strokeBorder(.black.opacity(0.4), lineWidth: 2))
                .overlay { label(size: hh / 2) }
                .frame(width: w, height: hh)
                .shadow(color: .black.opacity(pressed ? 0.25 : 0.55), radius: pressed ? 1 : 4, y: pressed ? 1 : 3)
        }
    }

    private var color: Color {
        switch layout.button {
        case .a: N64Palette.a
        case .b: N64Palette.b
        case .start: N64Palette.start
        case .cUp, .cDown, .cLeft, .cRight: N64Palette.c
        default: N64Palette.shell
        }
    }

    @ViewBuilder private func label(size r: CGFloat) -> some View {
        switch layout.button {
        case .a: letter("A", size: r)
        case .b: letter("B", size: r)
        case .l: letter("L", size: r, color: .black.opacity(0.55))
        case .r: letter("R", size: r, color: .black.opacity(0.55))
        case .z: letter("Z", size: r, color: .black.opacity(0.55))
        case .start:
            Text("START")
                .font(.system(size: r * 0.42, weight: .heavy, design: .rounded))
                .foregroundStyle(.white.opacity(0.85))
        case .cUp: arrow(r, degrees: 0)
        case .cRight: arrow(r, degrees: 90)
        case .cDown: arrow(r, degrees: 180)
        case .cLeft: arrow(r, degrees: 270)
        default: EmptyView()
        }
    }

    private func letter(_ text: String, size r: CGFloat, color: Color = .white.opacity(0.92)) -> some View {
        Text(text)
            .font(.system(size: r * 0.95, weight: .black, design: .rounded))
            .foregroundStyle(color)
    }

    private func arrow(_ r: CGFloat, degrees: Double) -> some View {
        Triangle()
            .fill(N64Palette.cArrow)
            .frame(width: r * 0.8, height: r * 0.66)
            .offset(y: -r * 0.04)
            .rotationEffect(.degrees(degrees))
    }
}

private struct DPadView: View {
    let layout: DPadLayout
    let model: N64Model

    var body: some View {
        let size = CGFloat(layout.size)
        let arm = size / 3
        ZStack {
            Cross()
                .fill(LinearGradient(
                    colors: [N64Palette.dpad.mix(with: .white, by: 0.15), N64Palette.dpad.mix(with: .black, by: 0.3)],
                    startPoint: .top, endPoint: .bottom))
                .shadow(color: .black.opacity(0.55), radius: 4, y: 3)
            ForEach(Self.arms, id: \.button) { item in
                Rectangle()
                    .fill(.black.opacity(model.isPressed(item.button) ? 0.35 : 0))
                    .frame(width: arm, height: arm)
                    .offset(x: item.dx * arm, y: item.dy * arm)
                Triangle()
                    .fill(.black.opacity(0.35))
                    .frame(width: arm * 0.42, height: arm * 0.32)
                    .rotationEffect(.degrees(item.degrees))
                    .offset(x: item.dx * arm * 1.05, y: item.dy * arm * 1.05)
            }
            Circle()
                .fill(.black.opacity(0.18))
                .frame(width: arm * 0.55, height: arm * 0.55)
        }
        .frame(width: size, height: size)
        .position(layout.center.cgPoint)
    }

    private struct Arm {
        let button: N64Button
        let dx: CGFloat
        let dy: CGFloat
        let degrees: Double
    }

    private static let arms = [
        Arm(button: .dUp, dx: 0, dy: -1, degrees: 0),
        Arm(button: .dRight, dx: 1, dy: 0, degrees: 90),
        Arm(button: .dDown, dx: 0, dy: 1, degrees: 180),
        Arm(button: .dLeft, dx: -1, dy: 0, degrees: 270),
    ]
}

private struct StickView: View {
    let layout: StickLayout
    let frame: N64Frame?

    var body: some View {
        let knob = CGFloat(layout.knobRadius)
        let reach = CGFloat(layout.diagonalTravel) + knob
        let gate = Octagon(cardinal: (CGFloat(layout.travel) + knob) / reach)
        let base = layout.center.cgPoint
        let tip = frame?.stickKnob.cgPoint ?? layout.center.cgPoint
        ZStack {
            gate
                .fill(RadialGradient(
                    colors: [N64Palette.gate, N64Palette.gate.mix(with: .white, by: 0.08)],
                    center: .center, startRadius: 0, endRadius: reach))
                .overlay(gate.stroke(.black.opacity(0.6), lineWidth: 2))
                .frame(width: reach * 2, height: reach * 2)
                .position(base)
            Circle()
                .fill(RadialGradient(
                    colors: [N64Palette.knob.mix(with: .white, by: 0.4), N64Palette.knob, N64Palette.knob.mix(with: .black, by: 0.35)],
                    center: UnitPoint(x: 0.38, y: 0.32), startRadius: 0, endRadius: knob * 2))
                .overlay(Circle().stroke(.black.opacity(0.18), lineWidth: 1.5).padding(knob * 0.28))
                .overlay(Circle().stroke(.black.opacity(0.14), lineWidth: 1.5).padding(knob * 0.55))
                .frame(width: knob * 2, height: knob * 2)
                .shadow(color: .black.opacity(0.6), radius: knob * 0.2, y: knob * 0.15)
                .position(tip)
        }
    }
}

/// The four bytes a game would receive, and the stick: proof that Rust sees
/// what the thumbs do.
private struct InputReadout: View {
    let frame: N64Frame?

    var body: some View {
        let bytes = Array(frame?.wire ?? Data(count: 4))
        VStack(spacing: 2) {
            Text(bytes.map { String(format: "%02X", $0) }.joined(separator: " "))
            Text(String(format: "X %+3d  Y %+3d", frame?.stickX ?? 0, frame?.stickY ?? 0))
                .foregroundStyle(.white.opacity(0.5))
        }
        .font(.system(size: 12, weight: .semibold, design: .monospaced))
        .foregroundStyle(.white.opacity(0.8))
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.black.opacity(0.35), in: Capsule())
    }
}

private struct Triangle: Shape {
    func path(in rect: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: rect.midX, y: rect.minY))
            p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
            p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
            p.closeSubpath()
        }
    }
}

private struct Cross: Shape {
    func path(in rect: CGRect) -> Path {
        let arm = rect.width / 3
        let corner = arm * 0.18
        var path = Path()
        path.addRoundedRect(in: rect.insetBy(dx: arm, dy: 0), cornerSize: CGSize(width: corner, height: corner))
        path.addRoundedRect(in: rect.insetBy(dx: 0, dy: arm), cornerSize: CGSize(width: corner, height: corner))
        return path
    }
}

/// The stick's gate: eight notches, one per direction, the diagonal ones
/// further out like on the real pad. `cardinal` is the axis notches' distance
/// relative to the diagonal ones.
private struct Octagon: Shape {
    let cardinal: CGFloat

    func path(in rect: CGRect) -> Path {
        let outer = min(rect.width, rect.height) / 2
        return Path { p in
            for i in 0..<8 {
                let angle = Double(i) * .pi / 4
                let radius = i.isMultiple(of: 2) ? outer * cardinal : outer
                let point = CGPoint(x: rect.midX + radius * cos(angle), y: rect.midY + radius * sin(angle))
                if i == 0 { p.move(to: point) } else { p.addLine(to: point) }
            }
            p.closeSubpath()
        }
    }
}
