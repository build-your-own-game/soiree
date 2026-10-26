import SwiftUI

/// Drawing for what every pad-like preset shares: buttons, the D-pad and the
/// stick. Rust decides where they are and what is pressed.
enum Palette {
    static let body = Color(red: 0.13, green: 0.135, blue: 0.15)
    static let bodyLight = Color(red: 0.22, green: 0.225, blue: 0.245)
    static let dpad = Color(red: 0.36, green: 0.37, blue: 0.4)
    static let gate = Color(red: 0.07, green: 0.075, blue: 0.085)
    static let knob = Color(red: 0.78, green: 0.79, blue: 0.81)
    static let selection = Color.white
    static let warning = Color(red: 1, green: 0.4, blue: 0.35)
}

extension Color {
    /// From `0xRRGGBB`.
    init(rgb: UInt32) {
        self.init(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255)
    }
}

/// Dark text on light buttons (yellow, white, gray), white on the others.
private func ink(on rgb: UInt32) -> Color {
    let (r, g, b) = (Double((rgb >> 16) & 0xFF), Double((rgb >> 8) & 0xFF), Double(rgb & 0xFF))
    return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.6 ? .black.opacity(0.62) : .white.opacity(0.92)
}

struct ControlView: View {
    let control: Control
    let pressed: Bool
    let selected: Bool
    /// For the drawing pad's size buttons: a dot as wide as the line.
    var dot: CGFloat?

    var body: some View {
        ZStack {
            // A pressed button is hidden under the thumb: a halo wider than
            // the button shows around it.
            halo
                .opacity(pressed ? 1 : 0)
                .animation(release(Self.haloFade), value: pressed)
            shape
                .scaleEffect(pressed ? 0.94 : 1)
                .brightness(pressed ? -0.14 : 0)
                .animation(release(Self.buttonFade), value: pressed)
            if selected {
                outline
                    .stroke(Palette.selection, lineWidth: 3)
                    .frame(width: size.width + 12, height: size.height + 12)
            }
        }
        .position(control.center.cgPoint)
    }

    /// A press shows on the next frame. A release fades: a quick tap lasts
    /// about one frame, and a full-strength flash that short reads as
    /// flicker. The button springs back first, the halo fades after it.
    static let buttonFade = 0.06
    static let haloFade = 0.12

    private func release(_ duration: Double) -> Animation? {
        pressed ? nil : .easeOut(duration: duration)
    }

    private var color: Color { Color(rgb: control.color) }

    private var size: CGSize {
        switch control.shape {
        case let .circle(radius): CGSize(width: CGFloat(radius) * 2, height: CGFloat(radius) * 2)
        case let .capsule(width, height), let .roundedRect(width, height, _):
            CGSize(width: CGFloat(width), height: CGFloat(height))
        }
    }

    private var outline: AnyShape {
        switch control.shape {
        case .circle: AnyShape(Circle())
        case .capsule: AnyShape(Capsule())
        case let .roundedRect(_, _, corner): AnyShape(RoundedRectangle(cornerRadius: CGFloat(corner)))
        }
    }

    /// Kept small and soft: a big change on every press distracts.
    private var halo: some View {
        let grow = min(size.width, size.height) * 0.2 + 8
        return outline
            .fill(color.opacity(0.4))
            .frame(width: size.width + grow * 2, height: size.height + grow * 2)
            .mask(
                RadialGradient(
                    colors: [.white, .white.opacity(0)], center: .center,
                    startRadius: min(size.width, size.height) * 0.45,
                    endRadius: max(size.width, size.height) * 0.5 + grow))
    }

    private var shape: some View {
        let radius = min(size.width, size.height) / 2
        return outline
            .fill(RadialGradient(
                colors: [color.mix(with: .white, by: 0.3), color, color.mix(with: .black, by: 0.25)],
                center: UnitPoint(x: 0.38, y: 0.3), startRadius: 0, endRadius: max(size.width, size.height)))
            .overlay(outline.stroke(.black.opacity(0.4), lineWidth: max(1.5, radius * 0.07)))
            .overlay { label }
            .frame(width: size.width, height: size.height)
            .shadow(
                color: .black.opacity(pressed ? 0.35 : 0.55), radius: min(radius * 0.14, 6),
                y: pressed ? 1 : min(radius * 0.12, 5))
    }

    @ViewBuilder private var label: some View {
        let short = min(size.width, size.height)
        switch control.label {
        case let .text(text):
            Text(text)
                .font(.system(size: fontSize(text, short: short), weight: .black, design: .rounded))
                .foregroundStyle(ink(on: control.color))
                .minimumScaleFactor(0.5)
                .lineLimit(1)
                .padding(.horizontal, 6)
        case let .arrow(direction):
            Triangle()
                .fill(.black.opacity(0.58))
                .frame(width: short * 0.4, height: short * 0.33)
                .offset(y: -short * 0.02)
                .rotationEffect(.degrees(direction.degrees))
        case .blank:
            if let dot {
                Circle().fill(.white).frame(width: dot, height: dot)
            }
        }
    }

    private func fontSize(_ text: String, short: CGFloat) -> CGFloat {
        switch text.count {
        case ...2: short * 0.48
        case ...5: short * 0.24
        default: short * 0.2
        }
    }
}

extension Direction {
    var degrees: Double {
        switch self {
        case .up: 0
        case .right: 90
        case .down: 180
        case .left: 270
        }
    }
}

struct DPadView: View {
    let dpad: DPad
    /// Pressed ids, as a value: SwiftUI redraws the cross when it changes.
    let pressed: Set<UInt8>

    var body: some View {
        let size = CGFloat(dpad.size)
        let arm = size / 3
        let arms: [(UInt8, CGFloat, CGFloat, Direction)] = [
            (dpad.up, 0, -1, .up), (dpad.right, 1, 0, .right), (dpad.down, 0, 1, .down), (dpad.left, -1, 0, .left),
        ]
        let down = arms.filter { pressed.contains($0.0) }.map(\.3)
        ZStack {
            Cross()
                .fill(LinearGradient(
                    colors: [Palette.dpad.mix(with: .white, by: 0.15), Palette.dpad.mix(with: .black, by: 0.3)],
                    startPoint: .top, endPoint: .bottom))
                .shadow(color: .black.opacity(0.55), radius: 4, y: 3)
            // Lit, not darkened: the cross's own shading already darkens
            // its lower arm.
            Arms(directions: down)
                .fill(.white.opacity(0.32))
                .clipShape(Cross())
                .animation(down.isEmpty ? .easeOut(duration: ControlView.buttonFade) : nil, value: down)
            ForEach(arms, id: \.0) { _, dx, dy, direction in
                Triangle()
                    .fill(.black.opacity(0.35))
                    .frame(width: arm * 0.42, height: arm * 0.32)
                    .rotationEffect(.degrees(direction.degrees))
                    .offset(x: dx * arm * 1.05, y: dy * arm * 1.05)
            }
            Circle()
                .fill(.black.opacity(0.18))
                .frame(width: arm * 0.55, height: arm * 0.55)
        }
        .frame(width: size, height: size)
        .position(dpad.center.cgPoint)
    }
}

struct StickView: View {
    let stick: Stick
    let knob: Point?

    var body: some View {
        let knobRadius = CGFloat(stick.knobRadius)
        let reach = CGFloat(stick.diagonalTravel) + knobRadius
        let gate = Octagon(cardinal: stick.octagonal ? (CGFloat(stick.travel) + knobRadius) / reach : 1)
        let center = stick.center.cgPoint
        ZStack {
            gate
                .fill(RadialGradient(
                    colors: [Palette.gate, Palette.gate.mix(with: .white, by: 0.08)],
                    center: .center, startRadius: 0, endRadius: reach))
                .overlay(gate.stroke(.black.opacity(0.6), lineWidth: 2))
                .frame(width: reach * 2, height: reach * 2)
                .position(center)
            Circle()
                .fill(RadialGradient(
                    colors: [Palette.knob.mix(with: .white, by: 0.4), Palette.knob, Palette.knob.mix(with: .black, by: 0.35)],
                    center: UnitPoint(x: 0.38, y: 0.32), startRadius: 0, endRadius: knobRadius * 2))
                .overlay(Circle().stroke(.black.opacity(0.18), lineWidth: 1.5).padding(knobRadius * 0.28))
                .overlay(Circle().stroke(.black.opacity(0.14), lineWidth: 1.5).padding(knobRadius * 0.55))
                .frame(width: knobRadius * 2, height: knobRadius * 2)
                .shadow(color: .black.opacity(0.6), radius: knobRadius * 0.2, y: knobRadius * 0.15)
                .position(knob?.cgPoint ?? center)
        }
    }
}

struct Triangle: Shape {
    func path(in rect: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: rect.midX, y: rect.minY))
            p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
            p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
            p.closeSubpath()
        }
    }
}

/// The outer squares of the cross's pressed directions.
private struct Arms: Shape {
    let directions: [Direction]

    func path(in rect: CGRect) -> Path {
        let arm = rect.width / 3
        var path = Path()
        for direction in directions {
            let (column, row): (CGFloat, CGFloat) =
                switch direction {
                case .up: (1, 0)
                case .down: (1, 2)
                case .left: (0, 1)
                case .right: (2, 1)
                }
            path.addRect(CGRect(x: rect.minX + column * arm, y: rect.minY + row * arm, width: arm, height: arm))
        }
        return path
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

/// A stick's gate. Below 1, `cardinal` draws the N64's eight notches, the
/// diagonal ones reaching further; at 1 the gate is round.
private struct Octagon: Shape {
    let cardinal: CGFloat

    func path(in rect: CGRect) -> Path {
        let outer = min(rect.width, rect.height) / 2
        if cardinal >= 1 { return Circle().path(in: rect) }
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
