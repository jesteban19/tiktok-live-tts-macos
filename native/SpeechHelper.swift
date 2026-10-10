import Foundation
import AVFoundation

func emit(_ value: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: value), var line = String(data: data, encoding: .utf8) else { return }
    line += "\n"; FileHandle.standardOutput.write(Data(line.utf8))
}
// State and AVFoundation access are confined to the main queue.
final class SpeechController: NSObject, AVSpeechSynthesizerDelegate, @unchecked Sendable {
    let synth = AVSpeechSynthesizer()
    var ids: [ObjectIdentifier: String] = [:]
    override init() { super.init(); synth.delegate = self }
    func handle(_ data: [String: Any]) {
        let command = data["command"] as? String ?? ""
        switch command {
        case "voices":
            emit(["kind": "voices", "voices": AVSpeechSynthesisVoice.speechVoices().map { ["id": $0.identifier, "name": $0.name, "language": $0.language] }])
        case "speak":
            let id = data["id"] as? String ?? ""
            let utterance = AVSpeechUtterance(string: data["text"] as? String ?? "")
            let voiceID = data["voice"] as? String ?? ""
            if !voiceID.isEmpty {
                guard let voice = AVSpeechSynthesisVoice(identifier: voiceID) else { emit(["kind":"error", "id":id, "message":"La voz seleccionada ya no está instalada."]); return }
                utterance.voice = voice
            } else { utterance.voice = AVSpeechSynthesisVoice(language: "es-ES") }
            let rate = data["rate"] as? Double ?? 0.5
            utterance.rate = Float(min(0.65, max(0.3, rate)))
            utterance.volume = Float(min(1.0, max(0.0, data["volume"] as? Double ?? 1.0)))
            ids[ObjectIdentifier(utterance)] = id
            synth.speak(utterance)
        case "pause": _ = synth.pauseSpeaking(at: .immediate)
        case "resume": _ = synth.continueSpeaking()
        case "stop": _ = synth.stopSpeaking(at: .immediate)
        case "quit": _ = synth.stopSpeaking(at: .immediate); exit(0)
        default: emit(["kind":"error", "message":"Comando de voz desconocido."])
        }
    }
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didStart utterance: AVSpeechUtterance) { emit(["kind":"started", "id":ids[ObjectIdentifier(utterance)] ?? ""]) }
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) { done(utterance, "finished") }
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) { done(utterance, "cancelled") }
    func done(_ utterance: AVSpeechUtterance, _ kind: String) { let id = ids.removeValue(forKey:ObjectIdentifier(utterance)) ?? ""; emit(["kind":kind,"id":id]) }
}
let controller = SpeechController()
controller.handle(["command":"voices"])
// Blocking stdin is isolated; every AVFoundation call runs on the main queue.
DispatchQueue.global(qos: .userInitiated).async {
    while let line = readLine() {
        if let bytes = line.data(using: .utf8), let message = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any] {
            DispatchQueue.main.async { controller.handle(message) }
        }
    }
    DispatchQueue.main.async { controller.handle(["command":"quit"]) }
}
RunLoop.main.run()
