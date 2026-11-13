# Work packages (formal 2.4) as a unit of their own: `W` and four digits,
# a flat number space. Alone, they are the whole system:
#
#   import "work-packages.ncl"                         # W0012 2026 tax return
#
# Stacked on a system with a `pattern`, such as JD, each can link to one of
# its numbers, and that system's `scheme` stays the core's:
#
#   (import "jd.ncl") & (import "work-packages.ncl")   # W0012~11.13 2026 tax return
let xd = import "xd.ncl" in
let { slot, sep, literal, label, .. } = xd in
let number = [literal "W", slot { name = "work package", width = 4 }] in
{
  pattern
    | doc "The number of the system this stacks on, which links point to. None when alone."
    | not_exported
    | default
    = null,
  work_packages
    | doc "Work package numbers: W0012, or W0012~11.13 when linked."
    | not_exported
    =
      xd.pattern (
        number
        @ (if pattern == null then [] else [xd.link { target = pattern }])
        @ [sep.label, label]
      ),
  scheme | default = work_packages.scheme,
} | xd.System
