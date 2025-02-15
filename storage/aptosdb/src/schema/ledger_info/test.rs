// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use proptest::prelude::*;
use schemadb::{schema::fuzzing::assert_encode_decode, test_no_panic_decoding};

proptest! {
    #[test]
    fn test_encode_decode(
        epoch in any::<u64>(),
        ledger_info_with_sigs in any_with::<LedgerInfoWithSignatures>((1..10).into())
    ) {
        assert_encode_decode::<LedgerInfoSchema>(&epoch, &ledger_info_with_sigs);
    }
}

test_no_panic_decoding!(LedgerInfoSchema);
