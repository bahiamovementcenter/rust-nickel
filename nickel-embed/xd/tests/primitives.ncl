# Examples of xD's primitives, as std.test assertions. Evaluated by
# crates/xd-nickel/tests/primitives.rs, with nickel/ on the import path; the
# laws (render and parse agree with the core for every scheme) are property
# tests there.
let xd = import "xd.ncl" in
let jd = (import "jd.ncl").pattern in
let work_packages = ((import "jd.ncl") & (import "work-packages.ncl")).work_packages in

# Units stack: work packages on a system that isn't JD, a hex one.
let hex_system =
  {
    pattern
      | not_exported
      = xd.pattern [xd.slot { name = "box", width = 2, alphabet = xd.alphabets.hex }, xd.sep.label, xd.label],
    scheme = pattern.scheme,
  }
    | xd.System
in
let hex_work_packages = (hex_system & (import "work-packages.ncl")).work_packages in
let { slot, sep, literal, label, alphabets, .. } = xd in

# A literal before the first slot lowers into the core: W, then four digits.
let w_number = xd.pattern [literal "W", slot { name = "package", width = 4 }, sep.label, label] in

# Mixed alphabets: a decimal category, then a hex board slot (experiments E9).
let boards =
  xd.pattern [
    slot { name = "category", width = 2 },
    sep.within,
    slot { name = "board", width = 2, alphabet = alphabets.hex },
  ]
in

# Patterns are arrays, so they compose: JD's number with a `+` child marker.
let jd_parts = [
  slot { name = "area", width = 1, range = true },
  slot { name = "category", width = 1 },
  sep.within,
  slot { name = "id", width = 2 },
]
in
let with_child = xd.pattern (jd_parts @ [sep.child, slot { name = "child", width = 1 }]) in

[
  # JD
  jd.render [1] == "10-19",
  jd.render [1, 1] == "11",
  jd.render [1, 1, 11] == "11.11",
  jd.name [1, 1, 11] "Passport" == "11.11 Passport",
  jd.name [1, 1, 11] "" == "11.11",
  jd.parse "11.11 Passport" == { address = [1, 1, 11], label = "Passport" },
  jd.parse "10-19 Life admin" == { address = [1], label = "Life admin" },
  jd.parse "11" == { address = [1, 1] },
  # An area is only ever written as its range, and a range must be whole.
  jd.parse "1" == null,
  jd.parse "10-18" == null,
  jd.parse "11.1" == null,
  jd.parse "+ Wendy" == null,
  jd.write (jd.parse "11.11 Passport") == "11.11 Passport",
  ("11.11 Passport" | jd.Name) == "11.11 Passport",
  ("11.11" | jd.Numeral) == "11.11",

  # Work packages: the literal lowers into the core as the first level's separator.
  w_number.name [11] "Garden" == "W0011 Garden",
  w_number.parse "W0011 Garden" == { address = [11], label = "Garden" },
  w_number.parse "w0011" == null,
  w_number.scheme.levels == [{ name = "package", width = 4, separator = "W", range = false }],

  # Mixed alphabets work in Nickel; the core doesn't hold them yet.
  boards.render [11, 255] == "11.FF",
  boards.parse "11.ff" == { address = [11, 255] },
  boards.parse "1A.FF" == null,
  # Work packages stacked on JD.
  work_packages.parse "W0012~11.13 2026 tax return" == { address = [12], link = [1, 1, 13], label = "2026 tax return" },
  work_packages.parse "W0012 Taxes" == { address = [12], label = "Taxes" },
  work_packages.parse "W0012~10-19" == { address = [12], link = [1] },
  work_packages.parse "W0012~11.130" == null,
  work_packages.parse "W0012~" == null,
  work_packages.write { address = [12], link = [1, 1, 13], label = "2026 tax return" } == "W0012~11.13 2026 tax return",
  work_packages.write (work_packages.parse "W0013~21.04 Kitchen") == "W0013~21.04 Kitchen",
  # Alone, work packages are a system of their own, with no links.
  let alone = (import "work-packages.ncl").work_packages in
  alone.parse "W0012 Taxes" == { address = [12], label = "Taxes" } && alone.parse "W0012~11.13" == null,
  # A required link.
  let linked = xd.pattern [literal "W", slot { name = "wp", width = 4 }, xd.link { target = jd, required = true }] in
  linked.parse "W0001" == null && linked.parse "W0001~11.11" == { address = [1], link = [1, 1, 11] },
  # Stacked on the hex system instead.
  hex_work_packages.parse "W0001~FF Shelf" == { address = [1], link = [255], label = "Shelf" },
  hex_work_packages.parse "W0001~11.11" == null,
  with_child.render [1, 1, 63, 2] == "11.63+2",
  with_child.parse "11.63+2" == { address = [1, 1, 63, 2] },

  # Custom alphabets are read exactly, and lower to the core's `alphabet`.
  let b58 = xd.pattern [slot { name = "box", width = 1, alphabet = alphabets.base58, range = true }, sep.within, slot { name = "item", width = 2, alphabet = alphabets.base58 }] in
  b58.render [57] == "z.11-z.zz" && b58.scheme.alphabet == std.string.join "" alphabets.base58.digits,
]
|> std.test.assert_all
