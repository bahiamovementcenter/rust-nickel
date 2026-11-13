# xD's primitives and the contracts every configuration is checked against.
#
# Two layers (ADR 0004):
#
# - The primitives: alphabets, slots, separators and labels. A pattern is an
#   array of them, so patterns compose with `@`. `xd.pattern` turns one into
#   contracts (`Numeral`, `Name`), functions (`render`, `name`, `parse`) and the
#   `scheme` the Rust core loads. They are built on Nickel's standard library
#   (`std.string`, `std.contract`, `std.array`), so any program that embeds
#   Nickel can use them without xD's core.
# - The core's schema: `Scheme`, `Level` and `System`. They mirror
#   xd_core::Scheme, so a configuration that passes them deserializes into
#   it; xd_core::Scheme::validate still checks what a contract can't see as
#   easily (digits that clash with separators, capacity overflow).
let default_digits = std.string.characters "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ" in

# The characters the regex engine treats as syntax. Escaping only these keeps
# every pattern a valid regex whatever its separators are.
let regex_syntax = std.string.characters "\\.+*?()|[]{}^$#&-~" in
let escape = fun text =>
  text
  |> std.string.characters
  |> std.array.map (fun c => if std.array.elem c regex_syntax then "\\" ++ c else c)
  |> std.string.join ""
in

let unusable_digit = fun c => c == "/" || std.string.is_match "^\\s$" c in

let has_duplicates = fun xs =>
  std.array.length (std.array.sort_dedup std.string.compare xs) != std.array.length xs
in

let AlphabetIsUsable =
  std.contract.from_validator (fun a =>
    let folded = if a.fold_case then std.array.map std.string.lowercase a.digits else a.digits in
    if !std.array.all (fun d => std.string.length d == 1) a.digits then
      'Error { message = "each digit of an alphabet is one character" }
    else if std.array.length a.digits < 2 then
      'Error { message = "an alphabet needs at least two digits" }
    else if has_duplicates folded then
      'Error { message = "an alphabet can't repeat a digit (ignoring case when `fold_case` is set)" }
    else if std.array.any unusable_digit a.digits then
      'Error { message = "an alphabet can't contain `/` or whitespace: they can't be in a folder name" }
    else
      'Ok
  )
in

let Alphabet' =
  std.contract.Sequence [
    {
      digits
        | Array String
        | doc "The digits, lowest first: one character each.",
      fold_case
        | Bool
        | doc "Read letters in either case. Written as listed."
        | default
        = false,
    },
    AlphabetIsUsable,
  ]
in

let alphabets' = {
  of
    | String -> Dyn
    | doc "An alphabet from a string of digits, lowest first, read exactly as written."
    = fun s => { digits = std.string.characters s } | Alphabet',
  first
    | std.number.PosNat -> Dyn
    | doc "The first `n` of 0-9A-Z, letters read in either case: the core's default digits."
    = fun n => { digits = std.array.slice 0 n default_digits, fold_case = true } | Alphabet',
  decimal = first 10,
  hex = first 16,
  alnum
    | doc "0-9A-Z: the largest alphabet that is safe on case-insensitive disks."
    = first 36,
  base58
    | doc "Bitcoin's base58: no 0, O, I or l. Mixed case."
    = of "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
  base62
    | doc "Digits, then upper case, then lower case: byte order is digit order."
    = of "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
  base64url
    | doc "RFC 4648's file-name-safe base64. Its `-` needs another range separator."
    = of "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
}
in

let Slot' = {
  kind | [| 'slot |],
  name
    | std.string.NonEmpty
    | doc "What this level is called, e.g. \"area\".",
  width
    | std.number.PosNat
    | doc "How many digits the slot holds.",
  alphabet
    | Alphabet'
    | doc "Which characters the digits are.",
  range
    | Bool
    | doc "Written as the span of its children, as JD writes area 1 as 10-19."
    | default
    = false,
  range_separator
    | std.string.NonEmpty
    | doc "Between the first and last child, when `range` is set."
    | default
    = "-",
}
in

let Text' = {
  kind | [| 'text |],
  text | std.string.NonEmpty,
  role
    | [| 'within, 'range, 'label, 'child, 'link, 'disk, 'literal |]
    | doc "What the text means: one of the separators of formal 3.1, or a literal."
    | default
    = 'literal,
}
in

let Label' = { kind | [| 'label |] } in

let text = fun role t => { kind = 'text, text = t, role = role } | Text' in

let kind_of = fun part =>
  if std.is_record part && std.record.has_field "kind" part then part.kind else 'unknown
in

let PatternIsWellFormed =
  std.contract.from_validator (fun parts =>
    let kinds = std.array.map kind_of parts in
    let n = std.array.length parts in
    let slots = std.array.filter (fun p => kind_of p == 'slot) parts in
    let last_slot = std.array.fold_left (fun acc i => if std.array.at i kinds == 'slot then i else acc) (-1) (std.array.range 0 n) in
    let links = std.array.filter (fun i => std.array.at i kinds == 'link) (std.array.range 0 n) in
    if !std.array.all (fun k => std.array.elem k ['slot, 'text, 'link, 'label]) kinds then
      'Error {
        message = "a pattern is made of slots, text, links and labels",
        notes = ["Build its parts with xd.slot, xd.sep, xd.literal, xd.link and xd.label."],
      }
    else if std.array.length links > 1 then
      'Error { message = "a pattern can have one link at most" }
    else if links != [] && (std.array.first links != last_slot + 1) then
      'Error { message = "a link comes right after the last slot" }
    else if std.array.length slots == 0 then
      'Error { message = "a pattern needs at least one slot" }
    else if std.array.any (fun i => std.array.at i kinds == 'label) (std.array.range 0 (n - 1)) then
      'Error { message = "the label can only be the last part of a pattern" }
    else if std.array.last kinds == 'label && (n < 2 || std.array.at (n - 2) kinds != 'text) then
      'Error { message = "a label needs a separator before it, such as xd.sep.label" }
    else if (std.array.last slots).range then
      'Error { message = "the last slot can't be written as a range: it has no children" }
    else
      'Ok
  )
in

{
  alphabets = alphabets',

  slot
    | doc m%"
        A slot: a fixed number of digits from one alphabet. Decimal unless
        `alphabet` says otherwise.

        ```nickel
        xd.slot { name = "id", width = 2 }
        xd.slot { name = "board", width = 1, alphabet = xd.alphabets.hex }
        ```
      "%
    = fun spec => ({ kind = 'slot, alphabet | default = alphabets'.decimal } & spec) | Slot,

  sep
    | doc "The separators of formal 3.1, as pattern parts."
    = {
      within | doc "`.`: the next level inside one number, as in 11.11." = text 'within ".",
      range | doc "`-`: from … to, as in 10-19." = text 'range "-",
      label | doc "` `: the end of the number and the start of the label." = text 'label " ",
      child | doc "`+`: a child that takes no number." = text 'child "+",
      link | doc "`~`: a link to an ID in the other tree." = text 'link "~",
      disk | doc "`/`: the next level on disk." = text 'disk "/",
    },

  link
    | doc m%"
        A link to a number of another pattern, written after the last slot,
        as JD links a work package to an ID: `W0011~11.13`. Optional unless
        `required = true`; `parse` gives the target's address as `link`.

        ```nickel
        xd.link { target = (import "jd.ncl").pattern }
        ```
      "%
    = fun spec => { kind = 'link, separator | default = "~", required | default = false } & spec,

  literal
    | doc "Fixed text in a number, such as the `W` of the work package `W0011`."
    = fun t => text 'literal t,

  label
    | doc "The label: free text after the number, which a rename changes (formal 1.3)."
    = { kind = 'label } | Label',

  pattern
    | doc m%"
        Compiles a pattern, an array of slots, text and an optional label, into
        what xD works with. Each slot is one level of the address, from the top.
        An address of depth `k` (one value per slot, for the first `k` slots) is
        written as the parts up to its last slot.

        The result has:

        - `render values`: the number, e.g. `[1, 1, 11]` → `"11.11"`.
        - `name values label`: the number and its label, e.g. `"11.11 Passport"`.
        - `parse string`: `{ address, link, label }` (without `link` or
          `label` when there is none), or `null` for anything outside the
          number space. Plain data, so it exports to JSON for programs that
          embed Nickel.
        - `write entry`: the inverse of `parse`, so `write (parse s) == s`.
        - `Numeral` and `Name`: contracts for written numbers and names that
          `parse` accepts (`Number` is a reserved word).
        - `scheme`: the scheme for xD's core (see `Scheme`). Fails for a
          pattern the core can't hold yet: mixed alphabets or range
          separators, or anything after the last slot but a space and the
          label.
      "%
    = fun parts_ =>
      # Plain copies of the parts, read through their contracts once. Reading
      # a field that has a contract re-applies it, and `parse` reads the
      # alphabets on every call.
      let parts =
        (parts_ | PatternIsWellFormed)
        |> std.array.map (fun p =>
          if p.kind == 'slot then
            let a = p.alphabet in
            {
              kind = 'slot,
              name = p.name,
              width = p.width,
              range = p.range,
              range_separator = p.range_separator,
              alphabet = { digits = a.digits, fold_case = a.fold_case },
            }
          else if p.kind == 'text then
            { kind = 'text, text = p.text }
          else if p.kind == 'link then
            { kind = 'link, text = p.separator, target = p.target, required = p.required }
          else
            { kind = 'label }
        )
      in
      let positions =
        std.array.range 0 (std.array.length parts)
        |> std.array.filter (fun i => kind_of (std.array.at i parts) == 'slot)
      in
      let slots = std.array.map (fun i => std.array.at i parts) positions in
      let depth = std.array.length slots in
      let base = fun s => std.array.length s.alphabet.digits in
      let capacity = fun s => std.number.pow (base s) s.width in
      # The parts that write an address of depth k: everything up to its last slot.
      let upto = fun k => std.array.slice 0 (std.array.at (k - 1) positions + 1) parts in
      let has_label = kind_of (std.array.last parts) == 'label in
      let label_text =
        if has_label then (std.array.at (std.array.length parts - 2) parts).text else null
      in

      let write_digits = fun s value =>
        std.array.range 0 s.width
        |> std.array.map (fun i =>
          let place = std.number.pow (base s) (s.width - 1 - i) in
          std.array.at ((std.number.truncate (value / place)) % (base s)) s.alphabet.digits
        )
        |> std.string.join ""
      in
      let write = fun values =>
        let k = std.array.length values in
        upto k
        |> std.array.fold_left
          (fun acc part =>
            if part.kind == 'text then
              { out = acc.out ++ part.text, i = acc.i }
            else
              { out = acc.out ++ write_digits part (std.array.at acc.i values), i = acc.i + 1 }
          )
          { out = "", i = 0 }
        |> (fun acc => acc.out)
      in

      let digit_class = fun s =>
        let alternatives = s.alphabet.digits |> std.array.map escape |> std.string.join "|" in
        if s.alphabet.fold_case then "(?i:%{alternatives})" else "(?:%{alternatives})"
      in
      let regex_upto = fun k =>
        upto k
        |> std.array.map (fun part =>
          if part.kind == 'text then escape part.text else "(%{digit_class part}{%{std.string.from_number part.width}})"
        )
        |> std.string.join ""
      in
      # Digit lookup tables, one per slot, built once.
      let tables =
        slots
        |> std.array.map (fun s =>
          std.array.range 0 (base s)
          |> std.array.flat_map (fun i =>
            let d = std.array.at i s.alphabet.digits in
            let forms = if s.alphabet.fold_case then [std.string.uppercase d, std.string.lowercase d] else [d] in
            std.array.map (fun f => { field = f, value = i }) (std.array.sort_dedup std.string.compare forms)
          )
          |> std.record.from_array
        )
      in
      let bases = std.array.map base slots in
      let values_of = fun groups =>
        std.array.range 0 (std.array.length groups)
        |> std.array.map (fun i =>
          let table = std.array.at i tables in
          let b = std.array.at i bases in
          std.string.characters (std.array.at i groups)
          |> std.array.fold_left (fun acc c => acc * b + table."%{c}") 0
        )
      in
      # One matcher per depth, built once: the plain form, and the range form
      # for slots written as ranges.
      let matchers =
        std.array.range 1 (depth + 1)
        |> std.array.map (fun k =>
          let s = std.array.at (k - 1) slots in
          if s.range then
            let child = regex_upto (k + 1) in
            {
              range = true,
              find = std.string.find "^%{child}%{escape s.range_separator}%{child}$",
            }
          else
            { range = false, find = std.string.find "^%{regex_upto k}$" }
        )
      in
      let parse_at = fun k string =>
        let m = (std.array.at (k - 1) matchers).find string in
        if m.index != 0 then
          'None
        else if (std.array.at (k - 1) matchers).range then
          let lo = values_of (std.array.slice 0 (k + 1) m.groups) in
          let hi = values_of (std.array.slice (k + 1) (2 * k + 2) m.groups) in
          let parent = std.array.slice 0 k lo in
          if std.array.slice 0 k hi == parent
          && std.array.last lo == 0
          && std.array.last hi == capacity (std.array.at k slots)
          - 1 then
            'Some parent
          else
            'None
        else
          'Some (values_of m.groups)
      in
      let parse_number = fun string =>
        std.array.range 1 (depth + 1)
        |> std.array.fold_left
          (fun found k => if found == 'None then parse_at k string else found)
          'None
      in
      # Splits at the first `sep`: { before, after }, with `after` null when
      # `sep` isn't there.
      let split_first = fun sep string =>
        let m = std.string.find (escape sep) string in
        if m.index == -1 then
          { before = string, after = null }
        else
          let rest = m.index + std.string.length sep in
          let end = std.string.length string in
          {
            before = std.string.substring 0 m.index string,
            after = if rest == end then "" else std.string.substring rest end string,
          }
      in
      let split_label = fun name =>
        if !has_label then
          { number = name, label = "" }
        else
          let split = split_first label_text name in
          { number = split.before, label = if split.after == null then "" else split.after }
      in
      let link =
        std.array.filter (fun p => p.kind == 'link) parts
        |> match { [l] => l, _ => null }
      in
      let check_values = fun values =>
        let k = std.array.length values in
        if k == 0 || k > depth then
          std.fail_with "an address has one value per slot: 1 to %{std.string.from_number depth}, not %{std.string.from_number k}"
        else if std.array.any
          (fun i =>
            let v = std.array.at i values in
            !std.is_number v
            || !std.number.is_integer v
            || v < 0
            || v >= capacity (std.array.at i slots)
          )
          (std.array.range 0 k) then
          std.fail_with "a value doesn't fit its slot"
        else
          values
      in
      let render' = fun values_ =>
        let values = check_values values_ in
        let k = std.array.length values in
        let s = std.array.at (k - 1) slots in
        if s.range then
          let max = capacity (std.array.at k slots) - 1 in
          write (values @ [0]) ++ s.range_separator ++ write (values @ [max])
        else
          write values
      in
      let lower =
        let last_slot = std.array.last positions in
        let separator_before = fun j =>
          let start = if j == 0 then 0 else std.array.at (j - 1) positions + 1 in
          std.array.slice start (std.array.at j positions) parts
          |> std.array.map (fun p => p.text)
          |> std.string.join ""
        in
        let trailing = std.array.slice (last_slot + 1) (std.array.length parts) parts in
        let a = (std.array.first slots).alphabet in
        let n = std.array.length a.digits in
        let range_separators =
          slots
          |> std.array.filter (fun s => s.range)
          |> std.array.map (fun s => s.range_separator)
          |> std.array.sort_dedup std.string.compare
        in
        if std.array.any (fun s => s.alphabet != a) slots then
          std.fail_with "xD's core needs one alphabet for every slot, for now"
        else if std.array.length range_separators > 1 then
          std.fail_with "xD's core needs one range separator for every slot, for now"
        else if link != null then
          std.fail_with "xD's core can't hold links yet"
        else if trailing != [] && !(has_label && std.array.length trailing == 2 && label_text == " ") then
          std.fail_with "xD's core only writes a space and a label after the number, for now"
        else
          {
            base = n,
            range_separator = if range_separators == [] then "-" else std.array.first range_separators,
            levels =
              std.array.range 0 depth
              |> std.array.map (fun j =>
                let s = std.array.at j slots in
                { name = s.name, width = s.width, separator = separator_before j, range = s.range }
              ),
          }
          & (
            if a.fold_case && a.digits == std.array.slice 0 n default_digits then
              {}
            else
              { alphabet = std.string.join "" a.digits }
          )
      in
      let parse' = fun string =>
        let split = split_label string in
        let number = if link == null then { before = split.number, after = null } else split_first link.text split.number in
        let target = if number.after == null then null else link.target.parse number.after in
        let with_label = if split.label == "" then {} else { label = split.label } in
        if number.after != null && (target == null || std.record.has_field "label" target) then
          null
        else if number.after == null && link != null && link.required then
          null
        else
          parse_number number.before
          |> match {
            'Some found =>
              { address = found }
              & (if target == null then {} else { link = target.address })
              & with_label,
            'None => null,
          }
      in
      let write_entry = fun entry =>
        render' entry.address
        ++ (
          if link != null && link.required && !std.record.has_field "link" entry then
            std.fail_with "this pattern's link is required"
          else if std.record.has_field "link" entry then
            if link == null then
              std.fail_with "this pattern has no link"
            else
              link.text ++ link.target.render entry.link
          else
            ""
        )
        ++ (
          if std.record.has_field "label" entry && entry.label != "" then
            (if has_label then label_text else " ") ++ entry.label
          else
            ""
        )
      in
      let accepts = fun what ok =>
        std.contract.from_validator (fun s =>
          if !std.is_string s then
            'Error { message = "expected a string" }
          else if ok s then
            'Ok
          else
            'Error { message = "`%{s}` isn't %{what} of this pattern" }
        )
      in
      {
        render = render',
        name = fun values l => if l == "" then render' values else render' values ++ (if has_label then label_text else " ") ++ l,
        parse = parse',
        write = write_entry,
        Numeral = accepts "a number" (fun s => parse_number s != 'None),
        Name = accepts "a name" (fun s => parse' s != null),
        scheme = lower,
      },

  Alphabet = Alphabet',
  Slot = Slot',
  Text = Text',
  Label = Label',

  Level = {
    name | std.string.NonEmpty | doc "What this level is called, e.g. \"area\".",
    width
      | std.number.PosNat
      | doc "Digits this level adds to the number.",
    separator
      | String
      | doc "Written before this level's digits: \"\" for JD's category, \".\" for its ID."
      | default
      = "",
    range
      | Bool
      | doc "Written as the span of its children, as JD writes area 1 as 10-19."
      | default
      = false,
  },

  Scheme = {
    base
      | std.number.Nat
      | std.contract.from_predicate (fun b => b >= 2)
      | doc "How many digits there are.",
    alphabet
      | String
      | doc "The digits, lowest first, exactly `base` of them. Without one, digits are 0-9A-Z (case-insensitive) and the base is at most 36."
      | optional,
    range_separator
      | std.string.NonEmpty
      | doc "Between the first and last child of a level written as a range."
      | default
      = "-",
    levels
      | Array Level
      | std.array.NonEmpty
      | doc "From the top of the tree down.",
  },

  # Open (`..`): a system may carry fields of its own that xD ignores
  # (docs/extensibility.md, "Unsealed classes"). Scheme and Level stay
  # closed so a misspelled field is an error.
  System = {
    scheme | Scheme,
    ..
  },
}
