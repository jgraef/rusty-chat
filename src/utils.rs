pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T: AsRef<str>> IsEmpty for T {
    fn is_empty(&self) -> bool {
        str::is_empty(self.as_ref())
    }
}

pub fn non_empty<T: IsEmpty>(x: T) -> Option<T> {
    if x.is_empty() {
        None
    }
    else {
        Some(x)
    }
}
