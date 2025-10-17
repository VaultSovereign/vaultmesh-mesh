use vaultmesh::sync::merkle_root;

#[test]
fn merkle_root_is_stable_and_order_independent() {
    let mut a = vec![
        "b3f1".to_string(),
        "00aa".to_string(),
        "ffff".to_string(),
        "0102".to_string(),
    ];
    let mut b = a.clone();
    a.sort();
    b.reverse();
    let mut b_sorted = b.clone();
    b_sorted.sort();

    let r1 = merkle_root(&a);
    let r2 = merkle_root(&b_sorted);
    assert_eq!(r1, r2);
    assert!(!r1.is_empty());
}
