/// Get the mean from vec of numbers.
pub fn get_mean(nums: &[f64]) -> f64 {
    nums.iter().sum::<f64>() / nums.len() as f64
}

/// Calculate standard deviation from vec of numbers.
pub fn get_standard_deviation(nums: &[f64], sample: bool) -> f64 {
    let len = nums.len() as f64;
    let len = if sample { len - 1.0 } else { len as f64 };
    let mean = get_mean(nums);
    let iter = nums.iter();
    // Sum of Squares
    let sos = iter.fold(0.0, |sum, n| (n - mean).powf(2.0) + sum);
    let variance = sos / (len) as f64;

    (variance.sqrt() * 100.0).round() / 100.0
}

/// Calculate poisson probability.
pub fn get_poisson_probability(lambda: f64, interval: f64) -> f64 {
    1.0 / (lambda * std::f64::consts::E.powf(interval * lambda))
}

/// Round a float to the specified precision
pub fn get_rounded_by(num: f64, precision: u8) -> f64 {
    (num * 10.0_f64.powf(precision as f64)).round() / 10.0_f64.powf(precision as f64)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_mean() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(get_mean(&data), 3.0);
    }

    #[test]
    fn test_get_standard_deviation() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(get_standard_deviation(&data, true), 1.58);
        assert_eq!(get_standard_deviation(&data, false), 1.41);
    }

    #[test]
    fn test_get_poisson_distribution() {
        assert_eq!(get_poisson_probability(6.0, -2.0).round(), 27126.0);
    }

    #[test]
    fn test_get_rounded_by() {
        assert_eq!(get_rounded_by(10.467864583333325, 2), 10.47);
        assert_eq!(get_rounded_by(10.467864583333325, 5), 10.46786);
    }
}
