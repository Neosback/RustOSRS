use super::*;

#[test]
fn wanted_regions_start_at_the_camera_region() {
    let wanted = wanted_regions((50, 50), 2);
    assert_eq!(wanted.len(), 25);
    assert_eq!(wanted[0], (50, 50));
    let ring = |&(x, y): &RegionKey| (x - 50).abs().max((y - 50).abs());
    assert!(wanted.windows(2).all(|w| ring(&w[0]) <= ring(&w[1])));
}

#[test]
fn stale_regions_keep_one_ring_of_hysteresis() {
    let loaded = [(50, 50), (53, 50), (54, 50), (50, 47)];
    let mut stale = stale_regions(loaded.into_iter(), (50, 50), 2);
    stale.sort_unstable();
    assert_eq!(stale, vec![(54, 50)]);
}
