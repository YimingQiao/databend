// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::default::Default;
use std::sync::Arc;

use common_catalog::table::Table;
use common_catalog::table_context::TableContext;
use common_exception::ErrorCode;
use common_exception::Result;
use common_expression::types::string::StringColumnBuilder;
use common_expression::types::AnyType;
use common_expression::types::DataType;
use common_expression::types::NumberDataType;
use common_expression::types::NumberType;
use common_expression::types::StringType;
use common_expression::types::ValueType;
use common_expression::Chunk;
use common_expression::TableDataType;
use common_expression::TableField;
use common_expression::TableSchemaRefExt;
use common_expression::Value;
use common_meta_app::schema::TableIdent;
use common_meta_app::schema::TableInfo;
use common_meta_app::schema::TableMeta;
use tikv_jemalloc_ctl::epoch;

use crate::SyncOneBlockSystemTable;
use crate::SyncSystemTable;

macro_rules! set_value {
    ($stat:ident, $names:expr, $values:expr) => {
        let mib = $stat::mib()?;
        let value = mib.read()?;
        $names.put_slice($stat::name().as_bytes());
        $names.commit_row();
        $values.push(value as u64);
    };
}

pub struct MallocStatsTotalsTable {
    table_info: TableInfo,
}

impl SyncSystemTable for MallocStatsTotalsTable {
    const NAME: &'static str = "system.malloc_stats_totals";

    fn get_table_info(&self) -> &TableInfo {
        &self.table_info
    }

    fn get_full_data(&self, _ctx: Arc<dyn TableContext>) -> Result<Chunk> {
        let values = Self::build_columns().map_err(convert_je_err)?;
        Ok(Chunk::new_from_sequence(values, 6))
    }
}

impl MallocStatsTotalsTable {
    pub fn create(table_id: u64) -> Arc<dyn Table> {
        let schema = TableSchemaRefExt::create(vec![
            TableField::new("name", TableDataType::String),
            TableField::new("value", TableDataType::Number(NumberDataType::UInt64)),
        ]);

        let table_info = TableInfo {
            desc: "'system'.'malloc_stats_totals'".to_string(),
            name: "malloc_stats_totals".to_string(),
            ident: TableIdent::new(table_id, 0),
            meta: TableMeta {
                schema,
                engine: "SystemMetrics".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        SyncOneBlockSystemTable::create(MallocStatsTotalsTable { table_info })
    }

    fn build_columns()
    -> std::result::Result<Vec<(Value<AnyType>, DataType)>, Box<dyn std::error::Error>> {
        let mut names = StringColumnBuilder::with_capacity(6, 6 * 4);
        let mut values: Vec<u64> = vec![];

        let e = epoch::mib()?;
        e.advance()?;

        use tikv_jemalloc_ctl::stats::active;
        use tikv_jemalloc_ctl::stats::allocated;
        use tikv_jemalloc_ctl::stats::mapped;
        use tikv_jemalloc_ctl::stats::metadata;
        use tikv_jemalloc_ctl::stats::resident;
        use tikv_jemalloc_ctl::stats::retained;

        set_value!(active, names, values);
        set_value!(allocated, names, values);
        set_value!(retained, names, values);
        set_value!(mapped, names, values);
        set_value!(resident, names, values);
        set_value!(metadata, names, values);

        let names = StringType::upcast_column(names.build());
        let values = NumberType::<u64>::upcast_column(values.into());

        Ok(vec![
            (Value::Column(names), DataType::String),
            (
                Value::Column(values),
                DataType::Number(NumberDataType::UInt64),
            ),
        ])
    }
}

fn convert_je_err(je_err: Box<dyn std::error::Error>) -> ErrorCode {
    ErrorCode::Internal(format!("{}", je_err))
}
