# Johnny.Decimal as an xD configuration: the defaults. Matches xd_core::jd.
#
# Built from xD's primitives: an area digit written as the span of its
# categories (10-19), a category digit (11), then a two-digit ID after a dot
# (11.11), then a space and the label. `pattern` is there for programs that
# embed Nickel (`(import "jd.ncl").pattern.parse "11.11 Passport"`); the core
# reads `scheme`.
let xd = import "xd.ncl" in
let { slot, sep, label, .. } = xd in
{
  pattern
    | not_exported
    =
      xd.pattern [
        slot { name = "area", width = 1, range = true },
        slot { name = "category", width = 1 },
        sep.within,
        slot { name = "id", width = 2 },
        sep.label,
        label,
      ],
  scheme = pattern.scheme,
} | xd.System
