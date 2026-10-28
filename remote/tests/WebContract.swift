// An independent compatibility test of the generated native ABI and the
// documented JavaScript contract. Run by `cargo xtask web-contract`.
import Foundation
import JavaScriptCore

enum ContractFailure: Error { case mismatch(String) }

@main
struct WebContractChecks {
    static func require(_ condition: Bool, _ message: String) throws {
        if !condition { throw ContractFailure.mismatch(message) }
    }

    static func main() throws {
        try require(webHandlerName() == "telecommande", "handler changed")
        let commands = webCommands(json: """
            {"vibrate":"success","haptic":[{"time":20,"duration":9,"intensity":2,"sharpness":-1}],
             "motion":true,"orientation":"portrait","datagram":{"op":"open","id":7,"port":8095}}
            """)
        try require(commands.count == 5, "combined JavaScript command lost")
        for command in commands {
            switch command {
            case .vibrate(let haptic): try require(haptic == .success, "vibration")
            case .haptic(let events):
                try require(events == [WebHapticEvent(time: 10, duration: 5, intensity: 1, sharpness: 0)], "haptic bounds")
            case .motion(let enabled): try require(enabled, "motion")
            case .orientation(let allowed): try require(allowed == .portrait, "allowed orientation")
            case .openDatagram(let id, let port): try require(id == 7 && port == 8095, "UDP open")
            case .sendDatagram, .closeDatagram: throw ContractFailure.mismatch("unexpected command")
            }
        }
        let send = webCommands(json: "{\"datagram\":{\"op\":\"send\",\"id\":7,\"data\":[0,255,34]}}")
        guard case .sendDatagram(let id, let bytes) = send.first else { throw ContractFailure.mismatch("UDP send") }
        try require(id == 7 && bytes == Data([0, 255, 34]), "bytes changed")
        guard case .closeDatagram(let closed) = webCommands(json: "{\"datagram\":{\"op\":\"close\",\"id\":7}}").first
        else { throw ContractFailure.mismatch("UDP close") }
        try require(closed == 7, "close id")
        for invalid in ["{", "[null,null,true,null,null]", "{\"haptic\":[[0,0,1,0.5]]}",
            "{\"motion\":1}", "{\"orientation\":\"sideways\"}",
            "{\"datagram\":{\"op\":\"open\",\"id\":-1,\"port\":80}}",
            "{\"datagram\":{\"op\":\"open\",\"id\":1,\"port\":0}}",
            "{\"datagram\":{\"op\":\"send\",\"id\":1,\"data\":[256]}}"] {
            try require(webCommands(json: invalid).isEmpty, "invalid JavaScript accepted: \(invalid)")
        }

        let context = JSContext()!
        context.evaluateScript("""
            var window = this; var events = [];
            function CustomEvent(name, options) { this.type = name; this.detail = options.detail; }
            window.dispatchEvent = function(event) { events.push(event); };
            """)
        let name = "é\"\\\n</script>"
        context.evaluateScript(webBootstrap(player: Data(repeating: 17, count: 16), name: name, orientation: .landscapeLeft)!)
        try require(context.evaluateScript("window.telecommande.player")!.toString() == String(repeating: "11", count: 16), "player identity")
        try require(context.evaluateScript("window.telecommande.name")!.toString() == name, "JavaScript string escaping")
        context.evaluateScript("var originalInfo = window.telecommande;")
        context.evaluateScript(webOrientation(orientation: .portrait)!)
        try require(context.evaluateScript("window.telecommande === originalInfo")!.toBool(), "orientation replaced info object")
        try require(context.evaluateScript("window.telecommande.orientation")!.toString() == "portrait", "orientation field")
        try require(context.evaluateScript("events[0].type")!.toString() == "telecommande:orientation", "orientation event name")
        try require(context.evaluateScript("events[0].detail")!.toString() == "portrait", "orientation detail")
        for (orientation, expected) in [(WebOrientation.portrait, "portrait"), (.landscapeLeft, "landscapeLeft"), (.landscapeRight, "landscapeRight")] {
            context.evaluateScript(webOrientation(orientation: orientation)!)
            try require(context.evaluateScript("window.telecommande.orientation")!.toString() == expected, "orientation mapping")
            try require(context.evaluateScript("events[events.length - 1].detail")!.toString() == expected, "orientation event mapping")
        }
        context.evaluateScript("events = [events[0]];")
        context.evaluateScript(webMotion(sample: WebMotion(
            gravity: WebVector3(x: 0, y: -1, z: 0), rotation: WebVector3(x: 0.1, y: 0, z: 0),
            acceleration: WebVector3(x: 0, y: 0, z: 0), attitude: WebQuaternion(x: 0, y: 0, z: 0, w: 1), time: 12.5))!)
        try require(context.evaluateScript("events[1].type")!.toString() == "telecommande:motion", "motion event name")
        try require(context.evaluateScript("JSON.stringify(events[1].detail.gravity)")!.toString() == "[0,-1,0]", "motion vector layout")
        try require(context.evaluateScript("events[1].detail.time")!.toDouble() == 12.5, "motion clock")
        context.evaluateScript(webDatagram(id: 7, data: Data([0, 255, 34]))!)
        try require(context.evaluateScript("events[2].type")!.toString() == "telecommande:datagram", "datagram event name")
        try require(context.evaluateScript("events[2].detail.data")!.toString() == "AP8i", "base64 wire shape")
        try require(context.exception == nil, "JavaScript exception")
        try require(webBootstrap(player: Data([1]), name: "x", orientation: .portrait) == nil, "invalid identity accepted")
        if let destination = CommandLine.arguments.dropFirst().first {
            // These are produced by the real Swift ABI, then consumed by the
            // actual WASM SDK in both browser engines (web-contract-browser).
            let fixtures = [
                "bootstrap": webBootstrap(player: Data(repeating: 17, count: 16), name: name, orientation: .landscapeLeft)!,
                "name": name,
                "orientation": webOrientation(orientation: .portrait)!,
                "motion": webMotion(sample: WebMotion(
                    gravity: WebVector3(x: 0, y: -1, z: 0), rotation: WebVector3(x: 0.1, y: 0, z: 0),
                    acceleration: WebVector3(x: 0, y: 0, z: 0), attitude: WebQuaternion(x: 0, y: 0, z: 0, w: 1), time: 12.5))!,
                "datagram": webDatagram(id: 1, data: Data("native ↔ page".utf8))!,
                "handler": webHandlerName(),
            ]
            try JSONSerialization.data(withJSONObject: fixtures).write(to: URL(fileURLWithPath: destination))
        }
        print("Generated Swift / Rust / JavaScript contract: all compatibility and validation checks passed.")
    }
}
