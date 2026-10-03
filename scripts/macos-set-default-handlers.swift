#!/usr/bin/env swift
import AppKit
import UniformTypeIdentifiers
import Foundation

/// After install: bind Monolith as default opener for every extension in file-types.json.
/// Uses macOS 12+ NSWorkspace APIs (label: toOpenFileAt / toOpen).

let app = URL(fileURLWithPath: "/Applications/Monolith.app")
guard FileManager.default.fileExists(atPath: app.path) else {
  fputs("Monolith.app not found at /Applications\n", stderr)
  exit(1)
}

let typesURL = URL(fileURLWithPath: CommandLine.arguments.count > 1
  ? CommandLine.arguments[1]
  : FileManager.default.currentDirectoryPath + "/src/file-types.json")
let data = try Data(contentsOf: typesURL)
let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
let types = json["types"] as! [[String: Any]]
let groups = json["groups"] as! [String: [String: Any]]
let exts = types.compactMap { $0["ext"] as? String }

let group = DispatchGroup()
var failures = 0

for ext in exts {
  let file = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("monolith-bind.\(ext)")
  try? "x\n".write(to: file, atomically: true, encoding: .utf8)
  group.enter()
  NSWorkspace.shared.setDefaultApplication(at: app, toOpenFileAt: file) { err in
    if err != nil { failures += 1 }
    group.leave()
  }
}

var contentTypes = Set<String>()
for (_, meta) in groups {
  if let cts = meta["contentTypes"] as? [String] {
    contentTypes.formUnion(cts)
  }
}
for id in contentTypes {
  guard let t = UTType(id) else { continue }
  group.enter()
  NSWorkspace.shared.setDefaultApplication(at: app, toOpen: t) { err in
    if err != nil { failures += 1 }
    group.leave()
  }
}

_ = group.wait(timeout: .now() + 120)

var other = 0
for ext in exts {
  let file = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("monolith-bind.\(ext)")
  if let a = NSWorkspace.shared.urlForApplication(toOpen: file), a.lastPathComponent != "Monolith.app" {
    print("OTHER", ext, "->", a.lastPathComponent)
    other += 1
  }
}
print("bound \(exts.count) extensions; verify others=\(other); apiFailures=\(failures)")
exit(other == 0 ? 0 : 2)
