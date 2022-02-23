use clarity::Uint256;

const ONE_ETH_FLOAT: f64 = 1000000000000000000.;
const ONE_GWEI_FLOAT: f64 = 1000000000.;
const ONE_NOM_FLOAT: f64 = 1000000000000000000.;

/// TODO revisit this for higher precision while
/// still representing the number to the user as a float
/// this takes a number like 0.37 eth and turns it into wei
/// or any erc20 with arbitrary decimals
pub fn fraction_to_exponent(num: f64, exponent: u8) -> Uint256 {
    let mut res = num;
    // in order to avoid floating point rounding issues we
    // multiply only by 10 each time. this reduces the rounding
    // errors enough to be ignored
    for _ in 0..exponent {
        res *= 10f64
    }
    Uint256::from_u128(res as u128)
}

pub fn print_eth(input: Uint256) -> String {
    let float: f64 = input.to_string().parse().unwrap();
    let res = float / ONE_ETH_FLOAT;
    format!("{:.4}", res)
}

pub fn print_nom(input: Uint256) -> String {
    let float: f64 = input.to_string().parse().unwrap();
    let res = float / ONE_NOM_FLOAT;
    format!("{:.4}", res)
}

pub fn print_gwei(input: Uint256) -> String {
    let float: f64 = input.to_string().parse().unwrap();
    let res = float / ONE_GWEI_FLOAT;
    format!("{:}", res)
}

#[test]
fn even_f32_rounding() {
    use clarity::u256;
    let one_eth = u256!(1000000000000000000);
    let one_point_five_eth = u256!(1500000000000000000);
    let one_point_one_five_eth = u256!(1150000000000000000);
    let a_high_precision_number = u256!(1150100000000000000);
    let res = fraction_to_exponent(1f64, 18);
    assert_eq!(one_eth, res);
    let res = fraction_to_exponent(1.5f64, 18);
    assert_eq!(one_point_five_eth, res);
    let res = fraction_to_exponent(1.15f64, 18);
    assert_eq!(one_point_one_five_eth, res);
    let res = fraction_to_exponent(1.1501f64, 18);
    assert_eq!(a_high_precision_number, res);
}
