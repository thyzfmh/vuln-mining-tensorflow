use flashdb_rust::FlashBackend;

#[test]
fn harness_starts_from_a_compiling_crate() {
    use flashdb_rust::flash::MemFlash;
    let flash = MemFlash::new(4096, 4);
    assert_eq!(flash.sector_size(), 4096);
}
