//! Profile of the card computation (headless): where the milliseconds of one card go.
//! `cargo test -p fs-packages-app --release --test card_profile -- --ignored --nocapture`

mod common;

use std::time::Instant;

use common::{fake, model, package, v};

fn ms(t: Instant, per: usize) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0 / per as f64
}

#[test]
#[ignore = "profile: prints times, asserts nothing"]
fn profile_one_card() {
    let f = fake(Vec::new(), None);
    {
        let mut s = f.state.lock().unwrap();
        s.offered = (0..14)
            .map(|i| package(&format!("fs-program-{i:02}"), v(0, 1, i)))
            .collect();
    }
    let m = model(&f);
    assert_eq!(m.cards().len(), 14);
    let n = 200;
    let t = Instant::now();
    for _ in 0..n {
        let _ = m.cards();
    }
    println!(
        "compute 14 cards with the fake backend: {:.3} ms per card",
        ms(t, n * 14)
    );
    let t = Instant::now();
    for _ in 0..5 {
        let _ = fs_packages_engine::preferred_format();
    }
    println!(
        "system_format of the real engine (once per card): {:.3} ms per call",
        ms(t, 5)
    );
    let t = Instant::now();
    for _ in 0..n {
        let _ = fs_packages_core::language::from_environment();
    }
    println!("Lang::current: {:.4} ms per call", ms(t, n));
}
