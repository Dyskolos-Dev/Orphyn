//! Animation helper: Blitz has no CSS transition / animation engine and there is no
//! framer-motion, so movement is integrated in Rust and written into a signal that the RSX puts
//! in a `style` (`transform: translateY({y}%)`, `opacity: {o}`).
//!
//! `use_spring(target, springs::SMOOTH)` follows `target` with the same physics as
//! framer-motion's springs (`design/motion.ts`), at ~120 Hz, and stops ticking at rest.
//! `use_tween(target, durations::BASE, easings::OUT)` is the duration-based variant for CSS
//! `transition`s.
use std::rc::Rc;
use std::time::Duration;

use tokio::sync::Notify;

use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}

/// `springs` of `design/motion.ts`.
pub mod springs {
    use super::Spring;
    pub const SMOOTH: Spring = Spring {
        stiffness: 300.0,
        damping: 30.0,
        mass: 1.0,
    };
    pub const SNAPPY: Spring = Spring {
        stiffness: 500.0,
        damping: 38.0,
        mass: 0.8,
    };
    pub const BOUNCY: Spring = Spring {
        stiffness: 400.0,
        damping: 22.0,
        mass: 0.9,
    };
}

/// `durations` of `design/motion.ts` (seconds).
pub mod durations {
    pub const INSTANT: f64 = 0.1;
    pub const FAST: f64 = 0.16;
    pub const BASE: f64 = 0.24;
    pub const SLOW: f64 = 0.36;
    pub const SLOWER: f64 = 0.52;
}

/// `easings` of `design/motion.ts` (cubic-bezier control points).
pub mod easings {
    pub const STANDARD: [f64; 4] = [0.25, 0.1, 0.25, 1.0];
    pub const OUT: [f64; 4] = [0.22, 1.0, 0.36, 1.0];
    pub const IN: [f64; 4] = [0.4, 0.0, 1.0, 1.0];
    pub const IN_OUT: [f64; 4] = [0.65, 0.0, 0.35, 1.0];
}

const STEP: f64 = 1.0 / 120.0;

/// One integration step (semi-implicit Euler). Returns true at rest.
pub fn spring_step(x: &mut f64, v: &mut f64, target: f64, s: Spring, dt: f64) -> bool {
    let a = (-s.stiffness * (*x - target) - s.damping * *v) / s.mass;
    *v += a * dt;
    *x += *v * dt;
    let at_rest = (*x - target).abs() < 0.01 && v.abs() < 0.01;
    if at_rest {
        *x = target;
        *v = 0.0;
    }
    at_rest
}

/// Evaluates a CSS `cubic-bezier(p)` easing at progress `t` (0..1).
pub fn cubic_bezier(p: [f64; 4], t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let bez = |a: f64, b: f64, u: f64| {
        3.0 * (1.0 - u).powi(2) * u * a + 3.0 * (1.0 - u) * u * u * b + u * u * u
    };
    // Solve x(u) = t by bisection, then return y(u).
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        if bez(p[0], p[2], mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(p[1], p[3], (lo + hi) / 2.0)
}

/// A value that springs towards `target` whenever `target` changes. `motion: reduced` callers
/// pass a stiff spring or set the value directly.
pub fn use_spring(target: f64, spring: Spring) -> ReadSignal<f64> {
    let mut value = use_signal(|| target);
    let mut goal = use_signal(|| target);
    let wake = use_hook(|| Rc::new(Notify::new()));
    if *goal.peek() != target {
        goal.set(target);
        wake.notify_one();
    }
    use_future(move || {
        let wake = wake.clone();
        async move {
            let mut v = 0.0;
            loop {
                let t = *goal.peek();
                let mut x = *value.peek();
                if x == t && v == 0.0 {
                    // At rest: sleep until the target changes.
                    wake.notified().await;
                    continue;
                }
                spring_step(&mut x, &mut v, t, spring, STEP);
                value.set(x);
                tokio::time::sleep(Duration::from_secs_f64(STEP)).await;
            }
        }
    });
    value.into()
}

/// A value that eases towards `target` over `seconds` with a cubic-bezier curve.
pub fn use_tween(target: f64, seconds: f64, easing: [f64; 4]) -> ReadSignal<f64> {
    let mut value = use_signal(|| target);
    let mut goal = use_signal(|| (target, target, 0u64));
    let wake = use_hook(|| Rc::new(Notify::new()));
    if goal.peek().1 != target {
        let from = *value.peek();
        let n = goal.peek().2 + 1;
        goal.set((from, target, n));
        wake.notify_one();
    }
    use_future(move || {
        let wake = wake.clone();
        async move {
            let mut done = 0u64;
            loop {
                let (from, to, n) = *goal.peek();
                if n == done {
                    wake.notified().await;
                    continue;
                }
                let start = std::time::Instant::now();
                loop {
                    if goal.peek().2 != n {
                        break;
                    }
                    let t = start.elapsed().as_secs_f64() / seconds.max(0.001);
                    value.set(from + (to - from) * cubic_bezier(easing, t));
                    if t >= 1.0 {
                        done = n;
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs_f64(STEP)).await;
                }
            }
        }
    });
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_spring_settles_without_overshooting_much() {
        let (mut x, mut v) = (100.0, 0.0);
        let mut steps = 0;
        let mut min: f64 = 100.0;
        while !spring_step(&mut x, &mut v, 0.0, springs::SMOOTH, STEP) {
            min = min.min(x);
            steps += 1;
            assert!(steps < 2000);
        }
        // framer-motion's smooth spring is nearly critically damped: settles in < 1 s.
        assert!(steps < 120, "{steps}");
        assert!(min > -5.0, "{min}");
    }

    #[test]
    fn bezier_endpoints_and_shape() {
        assert!((cubic_bezier(easings::OUT, 0.0)).abs() < 1e-6);
        assert!((cubic_bezier(easings::OUT, 1.0) - 1.0).abs() < 1e-6);
        assert!(cubic_bezier(easings::OUT, 0.5) > 0.5);
    }
}
