import SwiftUI

/// What a layout draws under its buttons: the wheel for `tilt`, the TV for
/// `pointer`, the canvas. Nothing here takes touches.
struct LayoutBackdrop: View {
    let model: RemoteModel
    let layout: RemoteLayout

    var body: some View {
        let area = layout.area.cgRect
        Group {
            if layout.tilt {
                SteeringWheel(view: model.frame?.wheel)
                    .position(x: area.midX, y: area.midY + 8)
            }
            if layout.pointer {
                PointerTV(view: model.frame?.pointer)
                    .position(x: area.midX, y: area.midY - 6)
            }
            if let canvas = layout.canvas?.cgRect {
                DrawingCanvas(rect: canvas, strokes: model.strokes, live: model.frame?.liveStroke)
            }
        }
        .allowsHitTesting(false)
    }
}

/// A steering wheel turned as far as the phone is.
private struct SteeringWheel: View {
    let view: WheelView?

    var body: some View {
        let angle = Double(view?.angle ?? 0)
        VStack(spacing: 14) {
            ZStack {
                Circle().stroke(Color.white.opacity(0.85), lineWidth: 14)
                Circle().stroke(Color.black.opacity(0.35), lineWidth: 2).padding(7)
                // Spokes to the left, the right and down, like a car's.
                ForEach([90.0, 180.0, 270.0], id: \.self) { spoke in
                    Capsule()
                        .fill(Color.white.opacity(0.7))
                        .frame(width: 10, height: 62)
                        .offset(y: -31)
                        .rotationEffect(.degrees(spoke))
                }
                Circle().fill(Color.white.opacity(0.85)).frame(width: 30, height: 30)
                Capsule().fill(Palette.warning).frame(width: 6, height: 16).offset(y: -64)
            }
            .frame(width: 150, height: 150)
            .rotationEffect(.degrees(angle))
            Text(caption)
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.55))
        }
    }

    private var caption: String {
        guard let view, view.available else { return "Pas de capteur de mouvement ici" }
        return String(format: "%+.0f° · %+.0f %%", view.angle, view.steering * 100)
    }
}

/// The TV in miniature, with the cursor where the game would show it.
private struct PointerTV: View {
    let view: PointerView?

    var body: some View {
        let size = CGSize(width: 240, height: 135)
        VStack(spacing: 12) {
            ZStack(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: 8).fill(Color.black.opacity(0.55))
                RoundedRectangle(cornerRadius: 8).stroke(Color.white.opacity(0.5), lineWidth: 2)
                Circle()
                    .fill(Color(red: 0.23, green: 0.53, blue: 1))
                    .overlay(Circle().stroke(.white, lineWidth: 2))
                    .frame(width: 16, height: 16)
                    .position(
                        x: CGFloat(view?.x ?? 0.5) * size.width,
                        y: CGFloat(view?.y ?? 0.5) * size.height)
            }
            .frame(width: size.width, height: size.height)
            Text(view?.available == true ? "Visez la TV · glissez pour ajuster" : "Glissez le doigt pour viser")
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.55))
        }
    }
}

/// The canvas and its lines, drawn as curves through the midpoints of the
/// samples, so a fast line does not look like a polygon.
private struct DrawingCanvas: View {
    let rect: CGRect
    let strokes: [Stroke]
    let live: Stroke?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(Color(white: 0.97))
                .shadow(color: .black.opacity(0.5), radius: 6, y: 3)
            Canvas { context, _ in
                for stroke in strokes {
                    draw(stroke, in: &context)
                }
                if let live {
                    draw(live, in: &context)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 6))
        }
        .frame(width: rect.width, height: rect.height)
        .position(x: rect.midX, y: rect.midY)
    }

    private func draw(_ stroke: Stroke, in context: inout GraphicsContext) {
        let points = stroke.points.map { CGPoint(x: CGFloat($0.x) - rect.minX, y: CGFloat($0.y) - rect.minY) }
        guard let first = points.first else { return }
        let color = Color(rgb: stroke.color)
        let width = CGFloat(stroke.width)
        if points.count == 1 {
            let dot = CGRect(x: first.x - width / 2, y: first.y - width / 2, width: width, height: width)
            context.fill(Path(ellipseIn: dot), with: .color(color))
            return
        }
        var path = Path()
        path.move(to: first)
        for (a, b) in zip(points, points.dropFirst()) {
            path.addQuadCurve(to: CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2), control: a)
        }
        path.addLine(to: points[points.count - 1])
        context.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: width, lineCap: .round, lineJoin: .round))
    }
}
