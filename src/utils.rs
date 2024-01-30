pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for &str {
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        String::is_empty(&self)
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
