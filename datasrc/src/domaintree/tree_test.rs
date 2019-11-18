use crate::domaintree::{
    node_chain::NodeChain,
    tree::{FindResultFlag, RBTree},
};
use proptest::{collection::vec, prelude::*};
use r53::Name;
use std::collections::HashSet;

prop_compose! {
    fn arb_name()(
        mut labels in vec("[a-z0-9]{1,63}", 1..128).prop_map(|v| v.join(".")),
        ) -> String {
        if labels.len() >= 254 {
            labels.truncate(253);
        }
        labels
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    #[ignore]
    #[test]
    fn prop_test_insert_delete_batch(
        name_strings in vec(arb_name(), 100..1000)
    ) {
        //filter duplicate name
        let names = name_strings
        .into_iter()
        .map(|labels| Name::new(labels.as_ref()).expect("rand name isn't valid"))
        .collect::<Vec<Name>>();
        test_insert_delete_batch(names);
    }
}

pub fn test_insert_delete_batch(names: Vec<Name>) {
    let mut tree = RBTree::<usize>::new();
    let mut duplicate_name_index = HashSet::new();
    for (i, name) in names.iter().enumerate() {
        let (_, old) = tree.insert(name.clone(), Some(i));
        //Some(None) == non-terminal node is created
        //None == new node
        if let Some(Some(v)) = old {
            assert!(name.eq(&names[v]));
            duplicate_name_index.insert(v);
        }
    }

    //duplicate insert should return old value
    for (i, name) in names.iter().enumerate() {
        if !duplicate_name_index.contains(&i) {
            let (_, old) = tree.insert(name.clone(), Some(i));
            assert_eq!(old.unwrap(), Some(i));
        }
    }

    for (i, name) in names.iter().enumerate() {
        if !duplicate_name_index.contains(&i) {
            let mut node_chain = NodeChain::new(&tree);
            let result = tree.find_node(name, &mut node_chain);
            assert_eq!(result.flag, FindResultFlag::ExacatMatch);
            assert_eq!(result.node.get_value(), &Some(i));
        }
    }

    for (i, name) in names.iter().enumerate() {
        if !duplicate_name_index.contains(&i) {
            let mut node_chain = NodeChain::new(&tree);
            let result = tree.find_node(&name, &mut node_chain);
            let node = result.node;
            assert_eq!(tree.remove_node(node).unwrap(), i);
        }
    }

    assert_eq!(tree.len(), 0);
}
