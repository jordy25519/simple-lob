#![feature(test)]

extern crate test;
use test::Bencher;

use std::hint::black_box;
use std::time::Duration;

use simple_lob::{Market, OrderSide, LOB};

#[bench]
fn bench_random_orders(b: &mut Bencher) {
    b.iter(|| black_box(bench_2()));
}

#[bench]
fn bench_market_orders(b: &mut Bencher) {
    b.iter(|| black_box(bench_1()));
}

pub fn bench_1() {
    let mut lob = Market::default();
    for i in 1..=100_000_u32 {
        black_box(assert!(lob
            .submit_order(i, 1, 1.0_f32, OrderSide::Sell)
            .is_ok()));
    }
    for i in 1..=100_000_u32 {
        black_box(assert!(lob
            .submit_order(i, 1, 1.0_f32, OrderSide::Buy)
            .is_ok()));
    }
}

#[test]
fn bench_1_t() {
    use std::time::Instant;
    let mut diffs = vec![];
    for _ in 0..100 {
        let s_0 = Instant::now();
        black_box(bench_1());
        let s_1 = Instant::now();
        diffs.push(s_1 - s_0);
    }
    let s_m: Duration = diffs.iter().sum();
    println!("{:?}", s_m / diffs.len() as u32);
    assert!(false);
}

pub fn bench_2() {
    use rand::Rng;

    let mut lob = Market::default();
    for i in 1_u32..=10_000 {
        let price_r = rand::thread_rng().gen_range(1..10_000);
        black_box(assert!(lob
            .submit_order(i, 1, price_r as f32, OrderSide::Sell)
            .is_ok()));
    }

    for i in 1_u32..=10_000 {
        let price_r = rand::thread_rng().gen_range(1..10_000);
        black_box(assert!(lob
            .submit_order(i, 1, price_r as f32, OrderSide::Buy)
            .is_ok()));
    }
}

#[test]
fn bench_2_t() {
    use std::time::Instant;
    let mut diffs = vec![];
    for _ in 0..100 {
        let s_0 = Instant::now();
        black_box(bench_2());
        let s_1 = Instant::now();
        diffs.push(s_1 - s_0);
    }
    let s_m: Duration = diffs.iter().sum();
    println!("{:?}", s_m / diffs.len() as u32);
    assert!(false);
}
