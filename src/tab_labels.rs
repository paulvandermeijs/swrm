/// Pick a tab label that doesn't collide with any name already in use within
/// a workspace. Returns `base` if it's free, otherwise the smallest
/// `format!("{base} {n}")` for `n >= 2` that isn't taken.
pub fn unique_label(existing: &[&str], base: &str) -> String {
    if !existing.iter().any(|s| *s == base) {
        return base.to_string();
    }
    for n in 2u32.. {
        let candidate = format!("{base} {n}");
        if !existing.iter().any(|s| *s == candidate) {
            return candidate;
        }
    }
    unreachable!("u32 exhausted while picking a tab label")
}
