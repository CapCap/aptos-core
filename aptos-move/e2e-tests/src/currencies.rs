// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{account::Account, compile, executor::FakeExecutor};
use aptos_types::transaction::WriteSetPayload;

pub fn add_currency_to_system(
    executor: &mut FakeExecutor,
    currency_code_to_register: &str,
    dr_account: &Account,
    current_dr_sequence_number: u64,
) -> u64 {
    let mut dr_sequence_number = current_dr_sequence_number;

    {
        let compiled_script = {
            let script = "
            import 0x1.TransactionPublishingOption;
            main(config: signer) {
            label b0:
                TransactionPublishingOption.set_open_module(&config, false);
                return;
            }
            ";
            compile::compile_script(script, vec![])
        };

        let txn = dr_account
            .transaction()
            .script(compiled_script)
            .sequence_number(dr_sequence_number)
            .sign();

        executor.execute_and_apply(txn);
    };

    executor.new_block();

    dr_sequence_number += 1;

    let (compiled_module, module) = {
        let module = format!(
            r#"
            module 0x1.{} {{
                import 0x1.Diem;
                import 0x1.FixedPoint32;
                struct {currency_code} has store {{ x: bool }}
                public init(dr_account: &signer, tc_account: &signer) {{
                label b0:
                    Diem.register_SCS_currency<Self.{currency_code}>(
                        move(dr_account),
                        move(tc_account),
                        FixedPoint32.create_from_rational(1, 1),
                        100,
                        1000,
                        h"{currency_code_hex}"
                    );
                    return;
                }}
            }}
            "#,
            currency_code = currency_code_to_register,
            currency_code_hex = hex::encode(currency_code_to_register),
        );

        compile::compile_module(&module)
    };

    let txn = dr_account
        .transaction()
        .module(module)
        .sequence_number(dr_sequence_number)
        .sign();

    executor.execute_and_apply(txn);

    dr_sequence_number += 1;

    {
        let compiled_script = {
            let script = format!(
                r#"
            import 0x1.{currency_code};
            main(dr_account: signer, tc_account: signer) {{
            label b0:
                {currency_code}.init(&dr_account, &tc_account);
                return;
            }}
            "#,
                currency_code = currency_code_to_register
            );
            compile::compile_script(&script, vec![compiled_module])
        };

        let write_set_payload = WriteSetPayload::Script {
            execute_as: *Account::new_blessed_tc().address(),
            script: compiled_script,
        };
        let txn = dr_account
            .transaction()
            .write_set(write_set_payload)
            .sequence_number(dr_sequence_number)
            .sign();
        executor.execute_and_apply(txn);
    };

    executor.new_block();

    dr_sequence_number + 1
}
