import AppKit
import NotesCore
import SwiftUI

private struct Mark: Decodable, Equatable {
    let start: Int
    let end: Int
    let style: String
}

private struct RichText: Decodable, Equatable {
    let text: String
    let marks: [Mark]

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.text.utf16.elementsEqual(rhs.text.utf16) && lhs.marks == rhs.marks
    }
}

private struct Block: Decodable, Identifiable {
    let id: String
    let kind: String
    let content: RichText?
    let depth: Int?
    let checked: Bool?
    let rows: [[RichText]]?
}

private struct Snapshot: Decodable {
    let revision: UInt64
    let blocks: [Block]
}

private struct Update: Decodable {
    struct Patch: Decodable {
        let type: String
        let id: String?
        let index: Int?
        let block: Block?
    }

    let base_revision: UInt64
    let revision: UInt64
    let patches: [Patch]
}

private struct Field: Hashable {
    let id: String
    var row: Int? = nil
    var column: Int? = nil

    func replace(_ range: NSRange, _ text: String) -> [String: Any] {
        var operation: [String: Any] = [
            "type": "replace", "id": id, "start": range.location,
            "end": NSMaxRange(range), "text": text
        ]
        if let row, let column { operation["cell"] = ["row": row, "column": column] }
        return operation
    }
}

private struct TextChange {
    let range: NSRange
    let replacement: String

    init(from old: String, to current: String) {
        let before = Array(old.utf16)
        let after = Array(current.utf16)
        let low = UInt16(0xDC00)...UInt16(0xDFFF)
        var start = 0
        while start < min(before.count, after.count), before[start] == after[start] { start += 1 }
        if start > 0, (start < before.count && low.contains(before[start]))
            || (start < after.count && low.contains(after[start])) { start -= 1 }
        var end = before.count
        var newEnd = after.count
        while end > start, newEnd > start, before[end - 1] == after[newEnd - 1] {
            end -= 1
            newEnd -= 1
        }
        if end < before.count, low.contains(before[end]) {
            end += 1
            newEnd += 1
        }
        range = NSRange(location: start, length: end - start)
        replacement = String(decoding: after[start..<newEnd], as: UTF16.self)
    }

    func translate(_ offset: Int) -> Int {
        if offset <= range.location { return offset }
        let length = replacement.utf16.count
        if offset >= NSMaxRange(range) { return offset + length - range.length }
        return range.location + length
    }
}

@MainActor
private final class NativeText: NSTextView, NSTextViewDelegate {
    var field: Field?
    weak var model: EditorModel?
    var keptContent = RichText(text: "", marks: [])
    var projecting = false
    var inputDepth = 0
    var composing: Bool { inputDepth != 0 || hasMarkedText() }
    var started: TimeInterval?

    override func becomeFirstResponder() -> Bool {
        let focused = super.becomeFirstResponder()
        if focused { model?.active = field }
        return focused
    }

    override func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        inputDepth += 1
        super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange)
        inputDepth -= 1
    }

    override func insertText(_ string: Any, replacementRange: NSRange) {
        if !projecting && started == nil { started = ProcessInfo.processInfo.systemUptime }
        inputDepth += 1
        super.insertText(string, replacementRange: replacementRange)
        inputDepth -= 1
        if !composing { model?.finishInput(self) }
    }

    override func unmarkText() {
        inputDepth += 1
        super.unmarkText()
        inputDepth -= 1
        if !composing { model?.finishInput(self) }
    }

    override func keyDown(with event: NSEvent) {
        if model?.key(event, in: self) != true { super.keyDown(with: event) }
    }

    func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange,
                  replacementString: String?) -> Bool {
        if !projecting && !composing && started == nil { started = ProcessInfo.processInfo.systemUptime }
        return true
    }

    func textDidChange(_ notification: Notification) {
        if !projecting && !composing { model?.finishInput(self) }
    }

    func project(_ content: RichText) {
        guard content != keptContent, !composing else { return }
        guard let contentStorage = textContentStorage, let storage = contentStorage.textStorage else { return }
        let selection = selectedRange()
        let change = TextChange(from: string, to: content.text)
        projecting = true
        defer { projecting = false }
        let base = NSFont.systemFont(ofSize: 15)
        let length = content.text.utf16.count
        contentStorage.performEditingTransaction {
            storage.beginEditing()
            if change.range.length > 0 || !change.replacement.isEmpty {
                storage.replaceCharacters(in: change.range, with: change.replacement)
            }
            storage.setAttributes([.font: base, .foregroundColor: NSColor.textColor],
                                  range: NSRange(location: 0, length: length))
            let boundaries = Set(content.marks.flatMap { [$0.start, $0.end] }).sorted()
            for (start, end) in zip(boundaries, boundaries.dropFirst()) {
                var traits: NSFontTraitMask = []
                for mark in content.marks where mark.start <= start && end <= mark.end {
                    if mark.style == "bold" { traits.insert(.boldFontMask) }
                    if mark.style == "italic" { traits.insert(.italicFontMask) }
                }
                storage.addAttribute(.font, value: NSFontManager.shared.convert(base, toHaveTrait: traits),
                                     range: NSRange(location: start, length: end - start))
            }
            storage.endEditing()
        }
        if selection.location != NSNotFound {
            let start = min(change.translate(selection.location), length)
            let end = min(change.translate(NSMaxRange(selection)), length)
            setSelectedRange(NSRange(location: start, length: end - start))
        }
        keptContent = content
    }
}

@MainActor
private final class EditorModel: ObservableObject {
    @Published var blocks: [Block] = []
    @Published var status = "Opening fixture…"
    @Published var generation = 0
    private var editor: PrototypeEditor?
    private var revision: UInt64 = 0
    private var views: [Field: NativeText] = [:]
    private var deferred: [(field: Field, offset: Int)] = []
    private var focusAfterSplit: Field?
    private var replayMatches: Bool?
    private var typing: [Double] = []
    private var patches: [Double] = []
    var active: Field?

    func reset() throws {
        let opened = try PrototypeEditor.openFixture()
        let snapshot = try decode(Snapshot.self, opened.snapshot())
        editor = opened
        revision = snapshot.revision
        views.removeAll()
        active = nil
        deferred.removeAll()
        typing.removeAll()
        patches.removeAll()
        focusAfterSplit = nil
        replayMatches = nil
        blocks = snapshot.blocks
        generation += 1
        report()
    }

    func attach(_ view: NativeText, to field: Field) {
        views[field] = view
        if focusAfterSplit == field {
            focusAfterSplit = nil
            DispatchQueue.main.async { [weak view] in
                if let view { view.window?.makeFirstResponder(view) }
            }
        }
    }

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) throws -> T {
        try JSONDecoder().decode(type, from: Data(json.utf8))
    }

    private func project(_ update: Update) throws {
        guard update.base_revision == revision else { throw CocoaError(.coderInvalidValue) }
        let start = ProcessInfo.processInfo.systemUptime
        for patch in update.patches {
            if patch.type == "delete", let id = patch.id {
                blocks.removeAll { $0.id == id }
                views = views.filter { $0.key.id != id }
                if active?.id == id { active = nil }
            } else if let block = patch.block, let index = patch.index {
                if blocks.indices.contains(index) && blocks[index].id == block.id {
                    blocks[index] = block
                } else {
                    blocks.insert(block, at: index)
                }
                if let content = block.content {
                    views[Field(id: block.id)]?.project(content)
                }
                if let rows = block.rows {
                    for (row, cells) in rows.enumerated() {
                        for (column, cell) in cells.enumerated() {
                            views[Field(id: block.id, row: row, column: column)]?.project(cell)
                        }
                    }
                }
            }
        }
        revision = update.revision
        if !update.patches.isEmpty && !views.isEmpty {
            patches.append((ProcessInfo.processInfo.systemUptime - start) * 1000)
        }
        report()
    }

    private func apply(_ operations: [[String: Any]], external: Bool = false) throws {
        guard let editor else { return }
        let request = try JSONSerialization.data(withJSONObject: [
            "operations": operations, "revision": revision, "external": external
        ])
        try project(decode(Update.self, editor.apply(request: String(decoding: request, as: UTF8.self))))
    }

    func finishInput(_ view: NativeText) {
        guard let field = view.field, !view.projecting, !view.composing else { return }
        let started = view.started
        view.started = nil
        run {
            let value = view.string
            if !view.keptContent.text.utf16.elementsEqual(value.utf16) {
                let change = TextChange(from: view.keptContent.text, to: value)
                try apply([field.replace(change.range, change.replacement)])
                if let started { typing.append((ProcessInfo.processInfo.systemUptime - started) * 1000) }
                report()
            }
            while let target = deferred.first, views[target.field]?.composing != true {
                deferred.removeFirst()
                try applyExternal(target.field, offset: target.offset)
            }
        }
    }

    func history(redo: Bool) {
        guard let editor, !views.values.contains(where: { $0.composing }) else { return }
        run { try project(decode(Update.self, editor.history(revision: revision, redo: redo))) }
    }

    func toggle(_ id: String) { run { try apply([["type": "toggle", "id": id]]) } }

    func sendPatch(_ key: UInt16) {
        var target: Field?
        switch key {
        case 98: target = active
        case 100:
            if let active, let column = active.column, let row = active.row,
               let cells = blocks.first(where: { $0.id == active.id })?.rows?[row], cells.count > 1 {
                target = Field(id: active.id, row: row, column: (column + 1) % cells.count)
            }
        default: target = blocks.first { $0.id != active?.id && $0.content != nil }.map { Field(id: $0.id) }
        }
        guard let target else { return }
        if let view = views[target], view.composing {
            let marked = view.markedRange()
            deferred.append((target, marked.location == NSNotFound ? view.selectedRange().location : marked.location))
            report("Patch queued until IME ends.")
        } else {
            run { try applyExternal(target, offset: 0) }
        }
    }

    private func applyExternal(_ field: Field, offset: Int) throws {
        let units = Array((views[field]?.keptContent.text ?? "").utf16)
        var position = min(offset, units.count)
        if position < units.count, (0xDC00...0xDFFF).contains(Int(units[position])) { position -= 1 }
        try apply([field.replace(NSRange(location: position, length: 0), "✓ ")], external: true)
    }

    func key(_ event: NSEvent, in view: NativeText) -> Bool {
        if [97, 98, 100].contains(event.keyCode) { sendPatch(event.keyCode); return true }
        guard !view.composing, let field = view.field else { return false }
        let shift = event.modifierFlags.contains(.shift)
        if event.modifierFlags.contains(.command), event.charactersIgnoringModifiers?.lowercased() == "z" {
            history(redo: shift)
            return true
        }
        guard blocks.first(where: { $0.id == field.id })?.kind == "checklist" else { return false }
        if event.keyCode == 48 {
            run { try apply([["type": "indent", "id": field.id, "delta": shift ? -1 : 1]]) }
            return true
        }
        if [36, 76].contains(event.keyCode), !shift {
            let selection = view.selectedRange()
            let id = UUID().uuidString
            var operations: [[String: Any]] = selection.length == 0 ? [] : [field.replace(selection, "")]
            operations.append(["type": "split_checklist", "id": field.id, "offset": selection.location, "new_id": id])
            run {
                try apply(operations)
                focusAfterSplit = Field(id: id)
            }
            return true
        }
        return false
    }

    func replay() {
        guard !views.values.contains(where: { $0.composing }) else { return }
        run {
            guard let operations = Bundle.main.url(forResource: "operations-v1", withExtension: "json"),
                  let expected = Bundle.main.url(forResource: "expected-v1", withExtension: "json"),
                  let sequence = try JSONSerialization.jsonObject(with: Data(contentsOf: operations)) as? [[String: Any]]
            else { throw CocoaError(.fileReadCorruptFile) }
            try reset()
            for operation in sequence { try apply([operation]) }
            guard let editor else { throw CocoaError(.coderInvalidValue) }
            let actual = try JSONSerialization.jsonObject(with: Data(editor.snapshot().utf8))
            let wanted = try JSONSerialization.jsonObject(with: Data(contentsOf: expected))
            replayMatches = (actual as? NSDictionary)?.isEqual(wanted) == true
            report("Checking fixture projection…")
        }
    }

    func run(_ action: () throws -> Void) {
        do { try action() }
        catch { report(String(describing: error)) }
    }

    private func report(_ message: String = "") {
        let textKit = views.isEmpty ? "waiting" : views.values.allSatisfy { $0.textLayoutManager != nil } ? "active" : "fallback"
        status = "Revision \(revision) · TextKit 2 \(textKit) · \(message)\nEdit to projection: \(summary(typing)) · Patch: \(summary(patches))"
    }

    func reportAttached() {
        if let replayMatches {
            let fieldCount = blocks.reduce(0) { $0 + ($1.rows?.reduce(0) { $0 + $1.count } ?? 1) }
            if views.count == fieldCount {
                self.replayMatches = nil
                let projected = blocks.allSatisfy { block in
                    if let content = block.content {
                        return views[Field(id: block.id)]?.string.utf16.elementsEqual(content.text.utf16) == true
                    }
                    return block.rows?.enumerated().allSatisfy { row, cells in
                        cells.enumerated().allSatisfy { column, content in
                            let field = Field(id: block.id, row: row, column: column)
                            return views[field]?.string.utf16.elementsEqual(content.text.utf16) == true
                        }
                    } == true
                }
                report(replayMatches && projected ? "Fixture replay PASS." : "Fixture replay FAIL.")
                return
            }
        }
        if status.contains("TextKit 2 waiting") {
            let textKit = views.values.allSatisfy { $0.textLayoutManager != nil } ? "active" : "fallback"
            status = status.replacingOccurrences(of: "TextKit 2 waiting", with: "TextKit 2 \(textKit)")
        }
    }

    private func summary(_ values: [Double]) -> String {
        guard !values.isEmpty else { return "0 samples" }
        let sorted = values.sorted()
        return String(format: "%d samples, p50 %.3f ms, p95 %.3f ms", sorted.count,
                      sorted[Int(ceil(Double(sorted.count) * 0.5)) - 1],
                      sorted[Int(ceil(Double(sorted.count) * 0.95)) - 1])
    }
}

@MainActor
private struct NativeField: NSViewRepresentable {
    let field: Field
    let content: RichText
    let model: EditorModel

    func makeNSView(context: Context) -> NSScrollView {
        let storage = NSTextContentStorage()
        storage.textStorage = NSTextStorage()
        let layout = NSTextLayoutManager()
        storage.addTextLayoutManager(layout)
        let container = NSTextContainer(size: NSSize(width: 400, height: .greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layout.textContainer = container
        let view = NativeText(frame: NSRect(x: 0, y: 0, width: 400, height: 40), textContainer: container)
        view.field = field
        view.model = model
        view.delegate = view
        view.allowsUndo = false
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.isAutomaticDashSubstitutionEnabled = false
        view.isVerticallyResizable = true
        view.autoresizingMask = [.width]
        view.textContainerInset = NSSize(width: 6, height: 7)
        view.setAccessibilityLabel(field.column.map { "\(field.id) \(field.row ?? 0) \($0)" } ?? field.id)
        let scroll = NSScrollView()
        scroll.borderType = .bezelBorder
        scroll.documentView = view
        scroll.hasVerticalScroller = true
        view.project(content)
        model.attach(view, to: field)
        DispatchQueue.main.async { model.reportAttached() }
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        (scroll.documentView as? NativeText)?.project(content)
    }
}

@MainActor
struct EditorPrototype: View {
    let version: String
    @StateObject private var model = EditorModel()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text("Editor M0 · Rust \(version)").font(.title)
                HStack {
                    Button("Undo") { model.history(redo: false) }
                    Button("Redo") { model.history(redo: true) }
                    Button("Patch other (F6)") { model.sendPatch(97) }
                    Button("Patch active (F7)") { model.sendPatch(98) }
                    Button("Replay fixture") { model.replay() }
                }
                Text(model.status).font(.footnote.monospacedDigit())
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(model.blocks) { block in
                        if let content = block.content {
                            HStack(alignment: .top) {
                                if let checked = block.checked {
                                    Toggle("Toggle \(block.id)", isOn: Binding(
                                        get: { checked }, set: { _ in model.toggle(block.id) }
                                    )).labelsHidden()
                                }
                                NativeField(field: Field(id: block.id), content: content, model: model)
                                    .frame(height: block.kind == "checklist" ? 68 : 48)
                            }.padding(.leading, CGFloat(block.depth ?? 0) * 24)
                        } else if let rows = block.rows {
                            Grid(horizontalSpacing: 4, verticalSpacing: 4) {
                                ForEach(rows.indices, id: \.self) { row in
                                    GridRow {
                                        ForEach(rows[row].indices, id: \.self) { column in
                                            NativeField(field: Field(id: block.id, row: row, column: column),
                                                        content: rows[row][column], model: model).frame(height: 42)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }.id(model.generation)
            }.padding()
        }.task {
            if model.blocks.isEmpty { model.run { try model.reset() } }
        }
    }
}
