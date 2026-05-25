/// Used when spawning a new tab in a workspace — pass the existing tab labels
/// and the desired base (e.g. an agent name or `"terminal"`) to get a label
/// that won't collide. Returns `base` when it's free, otherwise the smallest
/// `format!("{base} {n}")` for `n >= 2` that isn't taken. Treats `existing`
/// labels as opaque strings (no parsing): `"claude 2x"` is just "not equal to
/// `claude 2`" and doesn't block `claude 2` from being assigned.
pub fn unique_label(existing: &[impl AsRef<str>], base: &str) -> String {
    if !existing.iter().any(|s| s.as_ref() == base) {
        return base.to_string();
    }
    for n in 2u32.. {
        let candidate = format!("{base} {n}");
        if !existing.iter().any(|s| s.as_ref() == candidate) {
            return candidate;
        }
    }
    unreachable!("u32 exhausted while picking a tab label")
}
