pub fn remove_leading_zeroes(list: &[u8]) -> Vec<u8> {
    if let Some(first_non_zero) = list.iter().position(|&x| x != 0) {
        list[first_non_zero..].to_vec()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_remove_leading_zeroes() {
        // Test with leading zeroes
        let data = &[0, 0, 0, 1, 2, 3, 0, 4];
        let result = remove_leading_zeroes(data);
        assert_eq!(result, vec![1, 2, 3, 0, 4]);

        // Test with no leading zeroes
        let data = &[1, 0, 0, 1, 2, 3, 0, 4];
        let result = remove_leading_zeroes(data);
        assert_eq!(result, vec![1, 0, 0, 1, 2, 3, 0, 4]);

        // Test with all zeroes
        let data = &[0, 0, 0, 0, 0];
        let result = remove_leading_zeroes(data);
        assert_eq!(result, Vec::<u8>::new());

        // Test with an empty list
        let data = &[];
        let result = remove_leading_zeroes(data);
        assert_eq!(result, Vec::<u8>::new());
    }
}
