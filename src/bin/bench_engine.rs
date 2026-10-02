use std::time::Instant;
use ytm_shell::network_block::NetworkBlocker;

fn main() {
    let start = Instant::now();
    let blocker = NetworkBlocker::default_curated().expect("Failed to initialize engine");
    let load_duration = start.elapsed();

    println!("Engine Initialization Time: {:?}", load_duration);
    println!("Active Curated Rules: {}", blocker.rule_count());

    let test_urls = [
        (
            "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123",
            true,
        ),
        (
            "https://tpc.googlesyndication.com/sodar/5k7CCto5.html",
            true,
        ),
        ("https://www.google-analytics.com/analytics.js", true),
        (
            "https://music.youtube.com/youtubei/v1/log_event?alt=json",
            true,
        ),
        ("https://api.fingerprintjs.com/v3/get", true),
        (
            "https://rr1---sn-xxx.googlevideo.com/videoplayback?itag=251",
            false,
        ),
        ("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", false),
        ("https://music.youtube.com/", false),
    ];

    let match_start = Instant::now();
    let iterations = 10_000;
    for _ in 0..iterations {
        for (url, expected_block) in &test_urls {
            let blocked = blocker.check_network_request(
                url,
                "https://music.youtube.com/",
                "xmlhttprequest",
                "GET",
            );
            assert_eq!(blocked, *expected_block, "Assertion failed for {}", url);
        }
    }
    let match_duration = match_start.elapsed();
    let total_checks = iterations * test_urls.len();
    let per_check_micros = match_duration.as_micros() as f64 / total_checks as f64;

    println!(
        "Evaluated {} requests in {:?}",
        total_checks, match_duration
    );
    println!("Average check time: {:.2} µs/request", per_check_micros);
    println!("Blocked requests recorded: {}", blocker.blocked_count());
}
