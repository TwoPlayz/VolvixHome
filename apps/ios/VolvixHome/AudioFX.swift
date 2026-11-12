import Foundation
import AVFoundation
import MediaToolbox
import AudioToolbox
import CoreMedia

/// The equalizer's settings, shared by every song that is playing (during a crossfade that is two).
/// The main thread writes them; the audio thread reads them.
final class EQShared {
    static let shared = EQShared()

    let bands = 10
    /// per band: b0 b1 b2 a1 a2 (already divided by a0)
    let coef = UnsafeMutablePointer<Float>.allocate(capacity: 50)
    let active = UnsafeMutablePointer<Bool>.allocate(capacity: 10)
    var eqOn = false
    var preGain: Float = 1
    var sampleRate: Double = 44100

    init() {
        coef.initialize(repeating: 0, count: 50)
        active.initialize(repeating: false, count: 10)
    }

    /// Work out the filters (peaking filters, one per band) for the given gains in dB.
    func configure(on: Bool, gains: [Double], pre: Double, sampleRate fs: Double) {
        sampleRate = fs
        let q = 1.1
        for i in 0..<bands {
            let g = i < gains.count ? gains[i] : 0
            let f = eqBandFrequencies[i]
            if abs(g) < 0.05 || f >= fs / 2 * 0.95 {
                active[i] = false
                continue
            }
            let a = pow(10, g / 40)
            let w0 = 2 * Double.pi * f / fs
            let alpha = sin(w0) / (2 * q)
            let cosw = cos(w0)
            let a0 = 1 + alpha / a
            coef[i * 5 + 0] = Float((1 + alpha * a) / a0)
            coef[i * 5 + 1] = Float((-2 * cosw) / a0)
            coef[i * 5 + 2] = Float((1 - alpha * a) / a0)
            coef[i * 5 + 3] = Float((-2 * cosw) / a0)
            coef[i * 5 + 4] = Float((1 - alpha / a) / a0)
            active[i] = true
        }
        preGain = Float(pow(10, pre / 20))
        eqOn = on
    }
}

/// One song's sound processor: the equalizer, the preamp and the loudness level. Each song being played has its own
/// (the filters remember the last samples), but they all follow the same settings.
final class EQProcessor {
    /// loudness correction for this song (1 = none)
    var levelGain: Float = 1
    private var channels = 2
    private var interleaved = false
    private var isFloat = true
    private var z = UnsafeMutablePointer<Float>.allocate(capacity: 2 * 10 * 2)
    private var configuredRate: Double = 0

    init() {
        z.initialize(repeating: 0, count: 40)
    }

    deinit { z.deallocate() }

    func prepare(_ f: AudioStreamBasicDescription) {
        channels = max(1, min(2, Int(f.mChannelsPerFrame)))
        interleaved = (f.mFormatFlags & kAudioFormatFlagIsNonInterleaved) == 0
        isFloat = (f.mFormatFlags & kAudioFormatFlagIsFloat) != 0 && f.mBitsPerChannel == 32
        z.initialize(repeating: 0, count: 40)
        if f.mSampleRate > 0 && f.mSampleRate != configuredRate {
            configuredRate = f.mSampleRate
            EQShared.shared.sampleRate = f.mSampleRate
            NotificationCenter.default.post(name: .eqNeedsRate, object: nil)
        }
    }

    func process(_ list: UnsafeMutablePointer<AudioBufferList>, frames: Int) {
        guard isFloat else { return }
        let sh = EQShared.shared
        let eq = sh.eqOn
        let g = levelGain * (eq ? sh.preGain : 1)
        let bufs = UnsafeMutableAudioBufferListPointer(list)
        if !eq && g == 1 { return }
        for b in 0..<bufs.count {
            guard let raw = bufs[b].mData else { continue }
            let p = raw.assumingMemoryBound(to: Float.self)
            let chans = interleaved ? Int(bufs[b].mNumberChannels) : 1
            let n = frames
            for c in 0..<max(1, chans) {
                let ch = interleaved ? min(c, 1) : min(b, 1)
                let base = ch * 20
                for i in 0..<n {
                    let idx = i * chans + c
                    var x = p[idx] * g
                    if eq {
                        for band in 0..<10 where sh.active[band] {
                            let k = band * 5
                            let y = sh.coef[k] * x + z[base + band * 2]
                            z[base + band * 2] = sh.coef[k + 1] * x - sh.coef[k + 3] * y + z[base + band * 2 + 1]
                            z[base + band * 2 + 1] = sh.coef[k + 2] * x - sh.coef[k + 4] * y
                            x = y
                        }
                    }
                    p[idx] = max(-1, min(1, x))
                }
            }
        }
    }
}

extension Notification.Name {
    static let eqNeedsRate = Notification.Name("volvix.eq.needsRate")
}

// The callbacks the audio engine calls (they can't capture anything, so the processor rides in the tap's storage).
private let tapInit: MTAudioProcessingTapInitCallback = { _, clientInfo, storageOut in
    storageOut.pointee = clientInfo
}

private let tapFinalize: MTAudioProcessingTapFinalizeCallback = { tap in
    Unmanaged<EQProcessor>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).release()
}

private let tapPrepare: MTAudioProcessingTapPrepareCallback = { tap, _, format in
    let p = Unmanaged<EQProcessor>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).takeUnretainedValue()
    p.prepare(format.pointee)
}

private let tapUnprepare: MTAudioProcessingTapUnprepareCallback = { _ in }

private let tapProcess: MTAudioProcessingTapProcessCallback = { tap, frames, _, bufferList, framesOut, flagsOut in
    let status = MTAudioProcessingTapGetSourceAudio(tap, frames, bufferList, flagsOut, nil, framesOut)
    if status != noErr { return }
    let p = Unmanaged<EQProcessor>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).takeUnretainedValue()
    p.process(bufferList, frames: framesOut.pointee)
}

enum AudioFX {
    /// Put the equalizer / loudness processing on a song. Returns the processor so its level can be set.
    static func attach(to item: AVPlayerItem, levelDb: Double) async -> EQProcessor? {
        guard let track = try? await item.asset.loadTracks(withMediaType: .audio).first else { return nil }
        let proc = EQProcessor()
        proc.levelGain = Float(pow(10, levelDb / 20))
        var callbacks = MTAudioProcessingTapCallbacks(
            version: kMTAudioProcessingTapCallbacksVersion_0,
            clientInfo: Unmanaged.passRetained(proc).toOpaque(),
            init: tapInit,
            finalize: tapFinalize,
            prepare: tapPrepare,
            unprepare: tapUnprepare,
            process: tapProcess)
        var tap: Unmanaged<MTAudioProcessingTap>?
        let status = MTAudioProcessingTapCreate(kCFAllocatorDefault, &callbacks, kMTAudioProcessingTapCreationFlag_PostEffects, &tap)
        guard status == noErr, let t = tap else {
            Unmanaged<EQProcessor>.fromOpaque(callbacks.clientInfo!).release()
            return nil
        }
        let params = AVMutableAudioMixInputParameters(track: track)
        params.audioTapProcessor = t.takeRetainedValue()
        let mix = AVMutableAudioMix()
        mix.inputParameters = [params]
        item.audioMix = mix
        return proc
    }

    /// How much to turn a song up or down (dB) so songs sound equally loud, from its ReplayGain tags.
    static func levelDb(_ track: Track?, mode: String) -> Double {
        guard let rg = track?.replayGain, mode == "track" || mode == "album" else { return 0 }
        var db: Double?
        if mode == "album" { db = rg.album ?? rg.track } else { db = rg.track ?? rg.album }
        guard var d = db else { return 0 }
        // never boost past full scale: the peak says how much room there is
        if let peak = rg.peak, peak > 0, d > 0 { d = min(d, -20 * log10(peak)) }
        return max(-20, min(12, d))
    }
}
