import Foundation
import SwiftUI

func decimal(_ any: Any?) -> Double {
    if let d = any as? Double { return d }
    if let i = any as? Int { return Double(i) }
    if let s = any as? String, let d = Double(s) { return d }
    return 0
}

struct PartyMember: Hashable {
    var id: String
    var output: String
}

/// What the server says about a listening party: one shared queue and playing position.
struct PartyState: Equatable {
    var id: String
    var host: String
    var members: [PartyMember]
    var invited: [String]
    var queue: [String]
    var index: Int
    var playing: Bool
    var posMs: Double
    var updatedAt: Double
    var serverNow: Double
    var version: Int

    init?(_ o: [String: Any]?) {
        guard let o = o, let id = o["id"] as? String else { return nil }
        self.id = id
        host = (o["host"] as? String) ?? ""
        members = ((o["members"] as? [[String: Any]]) ?? []).map { PartyMember(id: ($0["id"] as? String) ?? "", output: ($0["output"] as? String) ?? "own") }
        invited = (o["invited"] as? [String]) ?? []
        queue = (o["queue"] as? [String]) ?? []
        index = wholeNumber(o["index"])
        playing = (o["playing"] as? Bool) ?? false
        posMs = decimal(o["posMs"])
        updatedAt = decimal(o["updatedAt"])
        serverNow = decimal(o["serverNow"])
        version = wholeNumber(o["version"])
    }
}

/// Listening parties: start one, invite people from your household, and everyone shares one queue that stays in sync.
/// The server holds the truth; this follows it and sends what you do.
@MainActor
final class PartyHub: ObservableObject {
    @Published private(set) var state: PartyState?
    @Published var notice: String?

    private weak var app: AppModel?
    private weak var player: PlayerModel?
    private var stream: Task<Void, Never>?
    private var started = false

    var active: Bool { state != nil }
    var myID: String { app?.user?.id ?? "" }
    var isHost: Bool { state?.host == myID }
    var output: String { state?.members.first { $0.id == myID }?.output ?? "own" }

    func name(_ id: String) -> String {
        if id == myID { return "You" }
        return app?.members.first { $0.id == id }?.name ?? "Someone"
    }

    func first(_ id: String) -> String {
        name(id).split(separator: " ").first.map(String.init) ?? name(id)
    }

    /// The people you can invite: everyone else on the server.
    var others: [Member] { (app?.members ?? []).filter { $0.id != myID } }

    func attach(_ app: AppModel, player: PlayerModel) {
        if started { return }
        started = true
        self.app = app
        self.player = player
        player.party = self
        Task { await findMine() }
    }

    /// A party you were in before the app closed.
    func findMine() async {
        guard let api = app?.api, app?.offline != true else { return }
        if let data = try? await api.send(try api.request("/api/party")),
           let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let s = PartyState(o["party"] as? [String: Any]) {
            join(state: s)
        }
    }

    // MARK: following the server

    private func join(state s: PartyState) {
        state = s
        listen(s.id)
        apply(s)
    }

    private func apply(_ s: PartyState) {
        if let cur = state, cur.id == s.id, s.version < cur.version { return }
        guard s.members.contains(where: { $0.id == myID }) else {
            detach("You're no longer in the party")
            return
        }
        state = s
        let offset = s.serverNow - Date().timeIntervalSince1970 * 1000
        player?.applyParty(s, output: output, offsetMs: offset)
    }

    private func detach(_ message: String?) {
        stream?.cancel()
        stream = nil
        guard state != nil else { return }
        state = nil
        player?.releaseParty()
        if let m = message { notice = m }
        Task { await app?.pollOnce() }
    }

    private func listen(_ id: String) {
        stream?.cancel()
        stream = Task { [weak self] in
            while !Task.isCancelled {
                guard let me = self, me.state?.id == id, let api = me.app?.api else { return }
                await me.read(api, id)
                if Task.isCancelled { return }
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                guard let me2 = self, me2.state?.id == id, let api2 = me2.app?.api else { return }
                // the line dropped: is the party still there?
                if let data = try? await api2.send(try api2.request("/api/party")),
                   let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   (o["party"] == nil || o["party"] is NSNull) {
                    me2.detach("The party ended")
                    return
                }
            }
        }
    }

    private func read(_ api: API, _ id: String) async {
        guard var req = try? api.request("/api/party/\(api.encode(id))/events") else { return }
        req.timeoutInterval = 3600
        req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        do {
            let (bytes, response) = try await URLSession.shared.bytes(for: req)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else { return }
            for try await line in bytes.lines {
                if Task.isCancelled { return }
                guard line.hasPrefix("data:") else { continue }
                let body = line.dropFirst(5).trimmingCharacters(in: .whitespaces)
                guard let data = body.data(using: .utf8), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
                if (o["type"] as? String) == "ended" {
                    detach("The party ended")
                    return
                }
                if (o["type"] as? String) == "state", let s = PartyState(o["state"] as? [String: Any]) {
                    apply(s)
                }
            }
        } catch {
            // the connection dropped: listen() looks again in a few seconds
        }
    }

    // MARK: things you do

    private func parse(_ data: Data) -> PartyState? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return PartyState(o["party"] as? [String: Any])
    }

    /// Send what you did (play, pause, skip, add a song…). The party answers with the new state.
    func send(_ op: String, _ extra: [String: Any] = [:]) async {
        guard let api = app?.api, let id = state?.id else { return }
        var body = extra
        body["op"] = op
        do {
            let data = try await api.send(try api.request("/api/party/\(api.encode(id))/action", method: "POST", json: body))
            if let s = parse(data) { apply(s) }
        } catch {
            notice = error.localizedDescription
        }
    }

    func fire(_ op: String, _ extra: [String: Any] = [:]) {
        Task { await send(op, extra) }
    }

    func start(invite: [String]) async {
        guard let api = app?.api, let player = player, state == nil else { return }
        var body = player.partySnapshot()
        body["invite"] = invite
        do {
            let data = try await api.send(try api.request("/api/party", method: "POST", json: body))
            if let s = parse(data) {
                join(state: s)
                notice = invite.isEmpty ? "Party started" : "Party started. Invited " + invite.map { first($0) }.joined(separator: ", ")
            }
        } catch {
            notice = error.localizedDescription
        }
    }

    func join(partyID: String, output: String) async {
        guard let api = app?.api else { return }
        if state != nil { notice = "You're already in a party. Leave it first."; return }
        do {
            let data = try await api.send(try api.request("/api/party/\(api.encode(partyID))/join", method: "POST", json: ["output": output]))
            if let s = parse(data) {
                join(state: s)
                notice = output == "host" ? "Joined, listening on \(first(s.host))'s speaker" : "Joined the party"
            }
        } catch {
            notice = error.localizedDescription
        }
        await app?.pollOnce()
    }

    func leave() async {
        guard let api = app?.api, let id = state?.id else { return }
        let wasHost = isHost
        _ = try? await api.send(try api.request("/api/party/\(api.encode(id))/leave", method: "POST", json: [String: Any]()))
        detach(wasHost ? "Party ended" : "You left the party")
    }

    func invite(_ ids: [String]) async {
        guard let api = app?.api, let id = state?.id, !ids.isEmpty else { return }
        do {
            let data = try await api.send(try api.request("/api/party/\(api.encode(id))/invite", method: "POST", json: ["userIds": ids]))
            if let s = parse(data) { apply(s) }
            notice = "Invited " + ids.map { first($0) }.joined(separator: ", ")
        } catch {
            notice = error.localizedDescription
        }
    }

    func setOutput(_ o: String) async {
        guard let api = app?.api, let id = state?.id else { return }
        do {
            let data = try await api.send(try api.request("/api/party/\(api.encode(id))/output", method: "POST", json: ["output": o]))
            if let s = parse(data) {
                apply(s)
                notice = o == "host" ? "This phone is silent: music comes from \(first(s.host))'s speaker" : "Playing on this phone"
            }
        } catch {
            notice = error.localizedDescription
        }
    }
}

// MARK: - Screens

/// The party sheet: start one, or see who is in it, invite people, choose where the sound plays, leave.
struct PartySheet: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var party: PartyHub
    @EnvironmentObject var player: PlayerModel
    @Environment(\.dismiss) private var dismiss
    @State private var chosen: Set<String> = []
    @State private var busy = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if let s = party.state {
                        active(s)
                    } else {
                        idle
                    }
                }
                .padding(16)
            }
            .background(Theme.surface)
            .navigationTitle("Listening party")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }

    private func personRow(_ m: Member, on: Bool, enabled: Bool = true, sub: String, toggle: @escaping () -> Void) -> some View {
        Button(action: toggle) {
            HStack(spacing: 12) {
                ZStack {
                    Circle().fill(Theme.tint(m.name))
                    Text(initials(m.name)).font(.system(size: 14, weight: .bold)).foregroundColor(.white)
                }
                .frame(width: 40, height: 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text(m.name).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                    Text(sub).font(.system(size: 12)).foregroundColor(Theme.text2)
                }
                Spacer()
                Image(systemName: on ? "checkmark.circle.fill" : "circle").font(.system(size: 22)).foregroundColor(on ? Theme.accent : Theme.text3)
            }
            .padding(10)
            .background(Theme.surface2)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .opacity(enabled ? 1 : 0.5)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
    }

    private var idle: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(player.hasCurrent
                 ? "What's playing now becomes the party's queue. Everyone you invite can add, remove and skip songs, and you all stay in sync."
                 : "Nothing is playing yet, so you'll start with an empty queue. Everyone you invite can add, remove and skip songs, and you all stay in sync.")
                .font(.system(size: 14)).foregroundColor(Theme.text2)
            if party.others.isEmpty {
                Text("No one else is on this server yet. You can still start a party and invite people later.").font(.system(size: 13)).foregroundColor(Theme.text2)
            }
            ForEach(party.others) { m in
                personRow(m, on: chosen.contains(m.id), sub: "Household") {
                    if chosen.contains(m.id) { chosen.remove(m.id) } else { chosen.insert(m.id) }
                }
            }
            PillButton(title: busy ? "Starting…" : "Start party", icon: "person.3.fill") {
                busy = true
                Task {
                    await party.start(invite: Array(chosen))
                    busy = false
                    chosen = []
                }
            }
            .disabled(busy)
        }
    }

    private func active(_ s: PartyState) -> some View {
        let inside = Set(s.members.map { $0.id } + s.invited)
        let canInvite = party.others.filter { !inside.contains($0.id) }
        return VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(party.isHost ? "You're the host" : "\(party.first(s.host)) is the host").font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.text)
                Text(party.output == "own" ? "Playing on this phone" : "Playing on \(party.first(s.host))'s speaker").font(.system(size: 13)).foregroundColor(Theme.text2)
            }
            Text("In the party").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase)
            ForEach(s.members, id: \.id) { m in
                HStack(spacing: 12) {
                    ZStack {
                        Circle().fill(Theme.tint(party.name(m.id)))
                        Text(initials(party.name(m.id))).font(.system(size: 13, weight: .bold)).foregroundColor(.white)
                    }
                    .frame(width: 36, height: 36)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(party.name(m.id)).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text(m.id == s.host ? "Host" : (m.output == "host" ? "Listening on the host's speaker" : "Listening on their own device"))
                            .font(.system(size: 12)).foregroundColor(Theme.text2)
                    }
                    Spacer()
                }
            }
            if !s.invited.isEmpty {
                Text("Invited: " + s.invited.map { party.name($0) }.joined(separator: ", ")).font(.system(size: 13)).foregroundColor(Theme.text2)
            }
            if !party.isHost {
                PillButton(title: party.output == "own" ? "Use \(party.first(s.host))'s speaker" : "Play on this phone", icon: "speaker.wave.2.fill", primary: false) {
                    Task { await party.setOutput(party.output == "own" ? "host" : "own") }
                }
            }
            if !canInvite.isEmpty {
                Text("Invite people").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 6)
                ForEach(canInvite) { m in
                    personRow(m, on: chosen.contains(m.id), sub: "Household") {
                        if chosen.contains(m.id) { chosen.remove(m.id) } else { chosen.insert(m.id) }
                    }
                }
                if !chosen.isEmpty {
                    PillButton(title: "Send invites", icon: "paperplane.fill") {
                        let ids = Array(chosen)
                        chosen = []
                        Task { await party.invite(ids) }
                    }
                }
            }
            Button(role: .destructive) {
                Task { await party.leave(); dismiss() }
            } label: {
                Text(party.isHost ? "End party for everyone" : "Leave party").font(.system(size: 15, weight: .semibold)).frame(maxWidth: .infinity).padding(.vertical, 12)
            }
            .padding(.top, 10)
        }
    }
}

/// Someone invited you: choose where the sound plays, then join.
struct JoinPartySheet: View {
    @EnvironmentObject var party: PartyHub
    @Environment(\.dismiss) private var dismiss
    let partyID: String
    let host: String
    @State private var output = "own"
    @State private var busy = false

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Where should the music play?").font(.system(size: 17, weight: .semibold)).foregroundColor(Theme.text)
                choice("own", "This phone", "You hear it here, in sync with everyone.")
                choice("host", "\(host)'s speaker", "This phone shows the party but stays silent. Best when you're in the same room or car.")
                Spacer()
                PillButton(title: busy ? "Joining…" : "Join", icon: "person.3.fill") {
                    busy = true
                    Task {
                        await party.join(partyID: partyID, output: output)
                        busy = false
                        dismiss()
                    }
                }
                .disabled(busy)
                .frame(maxWidth: .infinity)
            }
            .padding(20)
            .background(Theme.surface)
            .navigationTitle("Join \(host)'s party")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Not now") { dismiss() } } }
        }
        .presentationDetents([.medium])
    }

    private func choice(_ key: String, _ title: String, _ sub: String) -> some View {
        Button { output = key } label: {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: output == key ? "largecircle.fill.circle" : "circle").font(.system(size: 20)).foregroundColor(output == key ? Theme.accent : Theme.text3)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                    Text(sub).font(.system(size: 13)).foregroundColor(Theme.text2).multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
            .background(Theme.surface2)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}
