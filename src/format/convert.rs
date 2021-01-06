//! helper functions for conversion between internal structures and Protobuf
use crate::crypto::TokenSignature;
use curve25519_dalek::{ristretto::CompressedRistretto, scalar::Scalar};

use super::schema;
use crate::datalog::*;
use crate::error;
use crate::token::Block;

pub fn token_sig_to_proto_sig(input: &TokenSignature) -> schema::Signature {
    schema::Signature {
        parameters: input
            .parameters
            .iter()
            .map(|g| Vec::from(&g.compress().to_bytes()[..]))
            .collect(),
        z: Vec::from(&input.z.as_bytes()[..]),
    }
}

pub fn proto_sig_to_token_sig(input: schema::Signature) -> Result<TokenSignature, error::Format> {
    let mut parameters = vec![];

    for data in input.parameters {
        if data.len() == 32 {
            if let Some(d) = CompressedRistretto::from_slice(&data[..]).decompress() {
                parameters.push(d);
            } else {
                return Err(error::Format::DeserializationError(
                    "deserialization error: cannot decompress parameters point".to_string(),
                ));
            }
        } else {
            return Err(error::Format::DeserializationError(format!(
                "deserialization error: invalid size for parameters = {}",
                data.len()
            )));
        }
    }

    let z = if input.z.len() == 32 {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&input.z[..]);
        if let Some(d) = Scalar::from_canonical_bytes(bytes) {
            d
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: non canonical z scalar".to_string(),
            ));
        }
    } else {
        return Err(error::Format::DeserializationError(format!(
            "deserialization error: invalid size for z = {} bytes",
            input.z.len()
        )));
    };

    Ok(TokenSignature { parameters, z })
}

pub fn token_block_to_proto_block(input: &Block) -> schema::Block {
    schema::Block {
        index: input.index,
        symbols: input.symbols.symbols.clone(),
        facts_v0: Vec::new(),
        rules_v0: Vec::new(),
        caveats_v0: Vec::new(),
        context: input.context.clone(),
        version: Some(input.version),
        facts_v1: input
            .facts
            .iter()
            .map(v1::token_fact_to_proto_fact)
            .collect(),
        rules_v1: input
            .rules
            .iter()
            .map(v1::token_rule_to_proto_rule)
            .collect(),
        caveats_v1: input
            .caveats
            .iter()
            .map(v1::token_caveat_to_proto_caveat)
            .collect(),
    }
}

pub fn proto_block_to_token_block(input: &schema::Block) -> Result<Block, error::Format> {
    let version = input.version.unwrap_or(0);
    if version > crate::token::MAX_SCHEMA_VERSION {
        return Err(error::Format::Version {
            maximum: crate::token::MAX_SCHEMA_VERSION,
            actual: version,
        });
    }

    let mut facts = vec![];
    let mut rules = vec![];
    let mut caveats = vec![];
    if version == 0 {
        for fact in input.facts_v0.iter() {
            facts.push(v0::proto_fact_to_token_fact(fact)?);
        }

        for rule in input.rules_v0.iter() {
            rules.push(v0::proto_rule_to_token_rule(rule)?);
        }

        for caveat in input.caveats_v0.iter() {
            caveats.push(v0::proto_caveat_to_token_caveat(caveat)?);
        }
    } else {
        for fact in input.facts_v1.iter() {
            facts.push(v1::proto_fact_to_token_fact(fact)?);
        }

        for rule in input.rules_v1.iter() {
            rules.push(v1::proto_rule_to_token_rule(rule)?);
        }

        for caveat in input.caveats_v1.iter() {
            caveats.push(v1::proto_caveat_to_token_caveat(caveat)?);
        }
    }

    let context = input.context.clone();

    Ok(Block {
        index: input.index,
        symbols: SymbolTable {
            symbols: input.symbols.clone(),
        },
        facts,
        rules,
        caveats,
        context,
        version,
    })
}

pub mod v0 {
    use super::schema;
    use crate::datalog::*;
    use crate::error;

    pub fn token_fact_to_proto_fact(input: &Fact) -> schema::FactV0 {
        schema::FactV0 {
            predicate: token_predicate_to_proto_predicate(&input.predicate),
        }
    }

    pub fn proto_fact_to_token_fact(input: &schema::FactV0) -> Result<Fact, error::Format> {
        Ok(Fact {
            predicate: proto_predicate_to_token_predicate(&input.predicate)?,
        })
    }

    pub fn token_caveat_to_proto_caveat(input: &Caveat) -> schema::CaveatV0 {
        schema::CaveatV0 {
            queries: input.queries.iter().map(token_rule_to_proto_rule).collect(),
        }
    }

    pub fn proto_caveat_to_token_caveat(input: &schema::CaveatV0) -> Result<Caveat, error::Format> {
        let mut queries = vec![];

        for q in input.queries.iter() {
            queries.push(proto_rule_to_token_rule(q)?);
        }

        Ok(Caveat { queries })
    }

    pub fn token_rule_to_proto_rule(input: &Rule) -> schema::RuleV0 {
        schema::RuleV0 {
            head: token_predicate_to_proto_predicate(&input.head),
            body: input
                .body
                .iter()
                .map(token_predicate_to_proto_predicate)
                .collect(),
            constraints: input
                .constraints
                .iter()
                .map(token_constraint_to_proto_constraint)
                .collect(),
        }
    }

    pub fn proto_rule_to_token_rule(input: &schema::RuleV0) -> Result<Rule, error::Format> {
        let mut body = vec![];

        for p in input.body.iter() {
            body.push(proto_predicate_to_token_predicate(p)?);
        }

        let mut constraints = vec![];

        for c in input.constraints.iter() {
            constraints.push(proto_constraint_to_token_constraint(c)?);
        }

        Ok(Rule {
            head: proto_predicate_to_token_predicate(&input.head)?,
            body,
            constraints,
        })
    }

    pub fn token_predicate_to_proto_predicate(input: &Predicate) -> schema::PredicateV0 {
        schema::PredicateV0 {
            name: input.name,
            ids: input.ids.iter().map(token_id_to_proto_id).collect(),
        }
    }

    pub fn proto_predicate_to_token_predicate(
        input: &schema::PredicateV0,
    ) -> Result<Predicate, error::Format> {
        let mut ids = vec![];

        for id in input.ids.iter() {
            ids.push(proto_id_to_token_id(id)?);
        }

        Ok(Predicate {
            name: input.name,
            ids,
        })
    }

    pub fn token_id_to_proto_id(input: &ID) -> schema::Idv0 {
        use schema::idv0::Kind;

        match input {
            ID::Symbol(s) => schema::Idv0 {
                kind: Kind::Symbol as i32,
                symbol: Some(*s),
                ..Default::default()
            },
            ID::Variable(v) => schema::Idv0 {
                kind: Kind::Variable as i32,
                variable: Some(*v),
                ..Default::default()
            },
            ID::Integer(i) => schema::Idv0 {
                kind: Kind::Integer as i32,
                integer: Some(*i),
                ..Default::default()
            },
            ID::Str(s) => schema::Idv0 {
                kind: Kind::Str as i32,
                str: Some(s.clone()),
                ..Default::default()
            },
            ID::Date(d) => schema::Idv0 {
                kind: Kind::Date as i32,
                date: Some(*d),
                ..Default::default()
            },
            ID::Bytes(s) => schema::Idv0 {
                kind: Kind::Bytes as i32,
                bytes: Some(s.clone()),
                ..Default::default()
            },
        }
    }

    pub fn proto_id_to_token_id(input: &schema::Idv0) -> Result<ID, error::Format> {
        use schema::idv0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid id kind".to_string(),
            ));
        };

        match kind {
            Kind::Symbol => {
                if let Some(s) = input.symbol {
                    return Ok(ID::Symbol(s));
                }
            }
            Kind::Variable => {
                if let Some(v) = input.variable {
                    return Ok(ID::Variable(v));
                }
            }
            Kind::Integer => {
                if let Some(i) = input.integer {
                    return Ok(ID::Integer(i));
                }
            }
            Kind::Str => {
                if let Some(ref s) = input.str {
                    return Ok(ID::Str(s.clone()));
                }
            }
            Kind::Date => {
                if let Some(d) = input.date {
                    return Ok(ID::Date(d));
                }
            }
            Kind::Bytes => {
                if let Some(ref s) = input.bytes {
                    return Ok(ID::Bytes(s.clone()));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid id".to_string(),
        ))
    }

    pub fn token_constraint_to_proto_constraint(input: &Constraint) -> schema::ConstraintV0 {
        use schema::constraint_v0::Kind;

        match input.kind {
            ConstraintKind::Int(ref c) => schema::ConstraintV0 {
                id: input.id,
                kind: Kind::Int as i32,
                int: Some(token_int_constraint_to_proto_int_constraint(c)),
                ..Default::default()
            },
            ConstraintKind::Str(ref c) => schema::ConstraintV0 {
                id: input.id,
                kind: Kind::String as i32,
                str: Some(token_str_constraint_to_proto_str_constraint(c)),
                ..Default::default()
            },
            ConstraintKind::Date(ref c) => schema::ConstraintV0 {
                id: input.id,
                kind: Kind::Date as i32,
                date: Some(token_date_constraint_to_proto_date_constraint(c)),
                ..Default::default()
            },
            ConstraintKind::Symbol(ref c) => schema::ConstraintV0 {
                id: input.id,
                kind: Kind::Date as i32,
                symbol: Some(token_symbol_constraint_to_proto_symbol_constraint(c)),
                ..Default::default()
            },
            ConstraintKind::Bytes(ref c) => schema::ConstraintV0 {
                id: input.id,
                kind: Kind::Bytes as i32,
                bytes: Some(token_bytes_constraint_to_proto_bytes_constraint(c)),
                ..Default::default()
            },
        }
    }

    pub fn proto_constraint_to_token_constraint(
        input: &schema::ConstraintV0,
    ) -> Result<Constraint, error::Format> {
        use schema::constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::Int => {
                if let Some(ref i) = input.int {
                    return proto_int_constraint_to_token_int_constraint(i).map(|c| Constraint {
                        id: input.id,
                        kind: ConstraintKind::Int(c),
                    });
                }
            }
            Kind::String => {
                if let Some(ref i) = input.str {
                    return proto_str_constraint_to_token_str_constraint(i).map(|c| Constraint {
                        id: input.id,
                        kind: ConstraintKind::Str(c),
                    });
                }
            }
            Kind::Date => {
                if let Some(ref i) = input.date {
                    return proto_date_constraint_to_token_date_constraint(i).map(|c| Constraint {
                        id: input.id,
                        kind: ConstraintKind::Date(c),
                    });
                }
            }
            Kind::Symbol => {
                if let Some(ref i) = input.symbol {
                    return proto_symbol_constraint_to_token_symbol_constraint(i).map(|c| {
                        Constraint {
                            id: input.id,
                            kind: ConstraintKind::Symbol(c),
                        }
                    });
                }
            }
            Kind::Bytes => {
                if let Some(ref i) = input.bytes {
                    return proto_bytes_constraint_to_token_bytes_constraint(i).map(|c| {
                        Constraint {
                            id: input.id,
                            kind: ConstraintKind::Bytes(c),
                        }
                    });
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid constraint".to_string(),
        ))
    }

    pub fn token_int_constraint_to_proto_int_constraint(
        input: &IntConstraint,
    ) -> schema::IntConstraintV0 {
        use schema::int_constraint_v0::Kind;

        match input {
            IntConstraint::LessThan(i) => schema::IntConstraintV0 {
                kind: Kind::Lower as i32,
                lower: Some(*i),
                ..Default::default()
            },
            IntConstraint::GreaterThan(i) => schema::IntConstraintV0 {
                kind: Kind::Larger as i32,
                larger: Some(*i),
                ..Default::default()
            },
            IntConstraint::LessOrEqual(i) => schema::IntConstraintV0 {
                kind: Kind::LowerOrEqual as i32,
                lower_or_equal: Some(*i),
                ..Default::default()
            },
            IntConstraint::GreaterOrEqual(i) => schema::IntConstraintV0 {
                kind: Kind::LargerOrEqual as i32,
                larger_or_equal: Some(*i),
                ..Default::default()
            },
            IntConstraint::Equal(i) => schema::IntConstraintV0 {
                kind: Kind::Equal as i32,
                equal: Some(*i),
                ..Default::default()
            },
            IntConstraint::In(s) => schema::IntConstraintV0 {
                kind: Kind::In as i32,
                in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
            IntConstraint::NotIn(s) => schema::IntConstraintV0 {
                kind: Kind::NotIn as i32,
                not_in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
        }
    }

    pub fn proto_int_constraint_to_token_int_constraint(
        input: &schema::IntConstraintV0,
    ) -> Result<IntConstraint, error::Format> {
        use schema::int_constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid int constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::Lower => {
                if let Some(i) = input.lower {
                    return Ok(IntConstraint::LessThan(i));
                }
            }
            Kind::Larger => {
                if let Some(i) = input.larger {
                    return Ok(IntConstraint::GreaterThan(i));
                }
            }
            Kind::LowerOrEqual => {
                if let Some(i) = input.lower_or_equal {
                    return Ok(IntConstraint::LessOrEqual(i));
                }
            }
            Kind::LargerOrEqual => {
                if let Some(i) = input.larger_or_equal {
                    return Ok(IntConstraint::GreaterOrEqual(i));
                }
            }
            Kind::Equal => {
                if let Some(i) = input.equal {
                    return Ok(IntConstraint::Equal(i));
                }
            }
            Kind::In => {
                if !input.in_set.is_empty() {
                    return Ok(IntConstraint::In(input.in_set.iter().cloned().collect()));
                }
            }
            Kind::NotIn => {
                if !input.not_in_set.is_empty() {
                    return Ok(IntConstraint::NotIn(
                        input.not_in_set.iter().cloned().collect(),
                    ));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid id".to_string(),
        ))
    }

    pub fn token_str_constraint_to_proto_str_constraint(
        input: &StrConstraint,
    ) -> schema::StringConstraintV0 {
        use schema::string_constraint_v0::Kind;

        match input {
            StrConstraint::Prefix(s) => schema::StringConstraintV0 {
                kind: Kind::Prefix as i32,
                prefix: Some(s.clone()),
                ..Default::default()
            },
            StrConstraint::Suffix(s) => schema::StringConstraintV0 {
                kind: Kind::Suffix as i32,
                suffix: Some(s.clone()),
                ..Default::default()
            },
            StrConstraint::Equal(s) => schema::StringConstraintV0 {
                kind: Kind::Equal as i32,
                equal: Some(s.clone()),
                ..Default::default()
            },
            StrConstraint::Regex(r) => schema::StringConstraintV0 {
                kind: Kind::Regex as i32,
                regex: Some(r.clone()),
                ..Default::default()
            },
            StrConstraint::In(s) => schema::StringConstraintV0 {
                kind: Kind::In as i32,
                in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
            StrConstraint::NotIn(s) => schema::StringConstraintV0 {
                kind: Kind::NotIn as i32,
                not_in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
        }
    }

    pub fn proto_str_constraint_to_token_str_constraint(
        input: &schema::StringConstraintV0,
    ) -> Result<StrConstraint, error::Format> {
        use schema::string_constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid string constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::Prefix => {
                if let Some(ref s) = input.prefix {
                    return Ok(StrConstraint::Prefix(s.clone()));
                }
            }
            Kind::Suffix => {
                if let Some(ref s) = input.suffix {
                    return Ok(StrConstraint::Suffix(s.clone()));
                }
            }
            Kind::Equal => {
                if let Some(ref s) = input.equal {
                    return Ok(StrConstraint::Equal(s.clone()));
                }
            }
            Kind::Regex => {
                if let Some(ref r) = input.regex {
                    return Ok(StrConstraint::Regex(r.clone()));
                }
            }
            Kind::In => {
                if !input.in_set.is_empty() {
                    return Ok(StrConstraint::In(input.in_set.iter().cloned().collect()));
                }
            }
            Kind::NotIn => {
                if !input.not_in_set.is_empty() {
                    return Ok(StrConstraint::NotIn(
                        input.not_in_set.iter().cloned().collect(),
                    ));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid string constraint".to_string(),
        ))
    }

    pub fn token_date_constraint_to_proto_date_constraint(
        input: &DateConstraint,
    ) -> schema::DateConstraintV0 {
        use schema::date_constraint_v0::Kind;

        match input {
            DateConstraint::Before(i) => schema::DateConstraintV0 {
                kind: Kind::Before as i32,
                before: Some(*i),
                after: None,
            },
            DateConstraint::After(i) => schema::DateConstraintV0 {
                kind: Kind::After as i32,
                before: None,
                after: Some(*i),
            },
        }
    }

    pub fn proto_date_constraint_to_token_date_constraint(
        input: &schema::DateConstraintV0,
    ) -> Result<DateConstraint, error::Format> {
        use schema::date_constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid date constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::Before => {
                if let Some(i) = input.before {
                    return Ok(DateConstraint::Before(i));
                }
            }
            Kind::After => {
                if let Some(i) = input.after {
                    return Ok(DateConstraint::After(i));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid date constraint".to_string(),
        ))
    }

    pub fn token_symbol_constraint_to_proto_symbol_constraint(
        input: &SymbolConstraint,
    ) -> schema::SymbolConstraintV0 {
        use schema::symbol_constraint_v0::Kind;

        match input {
            SymbolConstraint::In(s) => schema::SymbolConstraintV0 {
                kind: Kind::In as i32,
                in_set: s.iter().cloned().collect(),
                not_in_set: vec![],
            },
            SymbolConstraint::NotIn(s) => schema::SymbolConstraintV0 {
                kind: Kind::NotIn as i32,
                in_set: vec![],
                not_in_set: s.iter().cloned().collect(),
            },
        }
    }

    pub fn proto_symbol_constraint_to_token_symbol_constraint(
        input: &schema::SymbolConstraintV0,
    ) -> Result<SymbolConstraint, error::Format> {
        use schema::symbol_constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid symbol constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::In => {
                if !input.in_set.is_empty() {
                    return Ok(SymbolConstraint::In(input.in_set.iter().cloned().collect()));
                }
            }
            Kind::NotIn => {
                if !input.not_in_set.is_empty() {
                    return Ok(SymbolConstraint::NotIn(
                        input.not_in_set.iter().cloned().collect(),
                    ));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid symbol constraint".to_string(),
        ))
    }

    pub fn token_bytes_constraint_to_proto_bytes_constraint(
        input: &BytesConstraint,
    ) -> schema::BytesConstraintV0 {
        use schema::bytes_constraint_v0::Kind;

        match input {
            BytesConstraint::Equal(s) => schema::BytesConstraintV0 {
                kind: Kind::Equal as i32,
                equal: Some(s.clone()),
                ..Default::default()
            },
            BytesConstraint::In(s) => schema::BytesConstraintV0 {
                kind: Kind::In as i32,
                in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
            BytesConstraint::NotIn(s) => schema::BytesConstraintV0 {
                kind: Kind::NotIn as i32,
                not_in_set: s.iter().cloned().collect(),
                ..Default::default()
            },
        }
    }

    pub fn proto_bytes_constraint_to_token_bytes_constraint(
        input: &schema::BytesConstraintV0,
    ) -> Result<BytesConstraint, error::Format> {
        use schema::bytes_constraint_v0::Kind;

        let kind = if let Some(i) = Kind::from_i32(input.kind) {
            i
        } else {
            return Err(error::Format::DeserializationError(
                "deserialization error: invalid bytes constraint kind".to_string(),
            ));
        };

        match kind {
            Kind::Equal => {
                if let Some(ref s) = input.equal {
                    return Ok(BytesConstraint::Equal(s.clone()));
                }
            }
            Kind::In => {
                if !input.in_set.is_empty() {
                    return Ok(BytesConstraint::In(input.in_set.iter().cloned().collect()));
                }
            }
            Kind::NotIn => {
                if !input.not_in_set.is_empty() {
                    return Ok(BytesConstraint::NotIn(
                        input.not_in_set.iter().cloned().collect(),
                    ));
                }
            }
        }

        Err(error::Format::DeserializationError(
            "deserialization error: invalid string constraint".to_string(),
        ))
    }
}

pub mod v1 {
    use super::schema;
    use crate::datalog::*;
    use crate::error;

    pub fn token_fact_to_proto_fact(input: &Fact) -> schema::FactV1 {
        schema::FactV1 {
            predicate: token_predicate_to_proto_predicate(&input.predicate),
        }
    }

    pub fn proto_fact_to_token_fact(input: &schema::FactV1) -> Result<Fact, error::Format> {
        Ok(Fact {
            predicate: proto_predicate_to_token_predicate(&input.predicate)?,
        })
    }

    pub fn token_caveat_to_proto_caveat(input: &Caveat) -> schema::CaveatV1 {
        schema::CaveatV1 {
            queries: input.queries.iter().map(token_rule_to_proto_rule).collect(),
        }
    }

    pub fn proto_caveat_to_token_caveat(input: &schema::CaveatV1) -> Result<Caveat, error::Format> {
        let mut queries = vec![];

        for q in input.queries.iter() {
            queries.push(proto_rule_to_token_rule(q)?);
        }

        Ok(Caveat { queries })
    }

    pub fn token_rule_to_proto_rule(input: &Rule) -> schema::RuleV1 {
        schema::RuleV1 {
            head: token_predicate_to_proto_predicate(&input.head),
            body: input
                .body
                .iter()
                .map(token_predicate_to_proto_predicate)
                .collect(),
            constraints: input
                .constraints
                .iter()
                .map(token_constraint_to_proto_constraint)
                .collect(),
        }
    }

    pub fn proto_rule_to_token_rule(input: &schema::RuleV1) -> Result<Rule, error::Format> {
        let mut body = vec![];

        for p in input.body.iter() {
            body.push(proto_predicate_to_token_predicate(p)?);
        }

        let mut constraints = vec![];

        for c in input.constraints.iter() {
            constraints.push(proto_constraint_to_token_constraint(c)?);
        }

        Ok(Rule {
            head: proto_predicate_to_token_predicate(&input.head)?,
            body,
            constraints,
        })
    }

    pub fn token_predicate_to_proto_predicate(input: &Predicate) -> schema::PredicateV1 {
        schema::PredicateV1 {
            name: input.name,
            ids: input.ids.iter().map(token_id_to_proto_id).collect(),
        }
    }

    pub fn proto_predicate_to_token_predicate(
        input: &schema::PredicateV1,
    ) -> Result<Predicate, error::Format> {
        let mut ids = vec![];

        for id in input.ids.iter() {
            ids.push(proto_id_to_token_id(id)?);
        }

        Ok(Predicate {
            name: input.name,
            ids,
        })
    }

    pub fn token_id_to_proto_id(input: &ID) -> schema::Idv1 {
        use schema::idv1::Content;

        match input {
            ID::Symbol(s) => schema::Idv1 {
                content: Some(Content::Symbol(*s)),
            },
            ID::Variable(v) => schema::Idv1 {
                content: Some(Content::Variable(*v)),
            },
            ID::Integer(i) => schema::Idv1 {
                content: Some(Content::Integer(*i)),
            },
            ID::Str(s) => schema::Idv1 {
                content: Some(Content::String(s.clone())),
            },
            ID::Date(d) => schema::Idv1 {
                content: Some(Content::Date(*d)),
            },
            ID::Bytes(s) => schema::Idv1 {
                content: Some(Content::Bytes(s.clone())),
            },
        }
    }

    pub fn proto_id_to_token_id(input: &schema::Idv1) -> Result<ID, error::Format> {
        use schema::idv1::Content;

        match &input.content {
            None => Err(error::Format::DeserializationError(
                "deserialization error: ID content enum is empty".to_string(),
            )),
            Some(Content::Symbol(i)) => Ok(ID::Symbol(*i)),
            Some(Content::Variable(i)) => Ok(ID::Variable(*i)),
            Some(Content::Integer(i)) => Ok(ID::Integer(*i)),
            Some(Content::String(s)) => Ok(ID::Str(s.clone())),
            Some(Content::Date(i)) => Ok(ID::Date(*i)),
            Some(Content::Bytes(s)) => Ok(ID::Bytes(s.clone())),
        }
    }

    pub fn token_constraint_to_proto_constraint(input: &Constraint) -> schema::ConstraintV1 {
        use schema::constraint_v1::Constraint;

        match input.kind {
            ConstraintKind::Int(ref c) => schema::ConstraintV1 {
                id: input.id,
                constraint: Some(Constraint::Int(
                    token_int_constraint_to_proto_int_constraint(c),
                )),
            },
            ConstraintKind::Str(ref c) => schema::ConstraintV1 {
                id: input.id,
                constraint: Some(Constraint::String(
                    token_str_constraint_to_proto_str_constraint(c),
                )),
            },
            ConstraintKind::Date(ref c) => schema::ConstraintV1 {
                id: input.id,
                constraint: Some(Constraint::Date(
                    token_date_constraint_to_proto_date_constraint(c),
                )),
            },
            ConstraintKind::Symbol(ref c) => schema::ConstraintV1 {
                id: input.id,
                constraint: Some(Constraint::Symbol(
                    token_symbol_constraint_to_proto_symbol_constraint(c),
                )),
            },
            ConstraintKind::Bytes(ref c) => schema::ConstraintV1 {
                id: input.id,
                constraint: Some(Constraint::Bytes(
                    token_bytes_constraint_to_proto_bytes_constraint(c),
                )),
            },
        }
    }

    pub fn proto_constraint_to_token_constraint(
        input: &schema::ConstraintV1,
    ) -> Result<Constraint, error::Format> {
        use schema::constraint_v1;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: constraint enum is empty".to_string(),
            )),
            Some(constraint_v1::Constraint::Int(i)) => {
                proto_int_constraint_to_token_int_constraint(i).map(|c| Constraint {
                    id: input.id,
                    kind: ConstraintKind::Int(c),
                })
            }
            Some(constraint_v1::Constraint::String(i)) => {
                proto_str_constraint_to_token_str_constraint(i).map(|c| Constraint {
                    id: input.id,
                    kind: ConstraintKind::Str(c),
                })
            }
            Some(constraint_v1::Constraint::Date(i)) => {
                proto_date_constraint_to_token_date_constraint(i).map(|c| Constraint {
                    id: input.id,
                    kind: ConstraintKind::Date(c),
                })
            }
            Some(constraint_v1::Constraint::Symbol(i)) => {
                proto_symbol_constraint_to_token_symbol_constraint(i).map(|c| Constraint {
                    id: input.id,
                    kind: ConstraintKind::Symbol(c),
                })
            }
            Some(constraint_v1::Constraint::Bytes(i)) => {
                proto_bytes_constraint_to_token_bytes_constraint(i).map(|c| Constraint {
                    id: input.id,
                    kind: ConstraintKind::Bytes(c),
                })
            }
        }
    }

    pub fn token_int_constraint_to_proto_int_constraint(
        input: &IntConstraint,
    ) -> schema::IntConstraintV1 {
        use schema::int_constraint_v1::Constraint;

        match input {
            IntConstraint::LessThan(i) => schema::IntConstraintV1 {
                constraint: Some(Constraint::LessThan(*i)),
            },
            IntConstraint::GreaterThan(i) => schema::IntConstraintV1 {
                constraint: Some(Constraint::GreaterThan(*i)),
            },
            IntConstraint::LessOrEqual(i) => schema::IntConstraintV1 {
                constraint: Some(Constraint::LessOrEqual(*i)),
            },
            IntConstraint::GreaterOrEqual(i) => schema::IntConstraintV1 {
                constraint: Some(Constraint::GreaterOrEqual(*i)),
            },
            IntConstraint::Equal(i) => schema::IntConstraintV1 {
                constraint: Some(Constraint::Equal(*i)),
            },
            IntConstraint::In(s) => schema::IntConstraintV1 {
                constraint: Some(Constraint::InSet(schema::IntSet {
                    set: s.iter().cloned().collect(),
                })),
            },
            IntConstraint::NotIn(s) => schema::IntConstraintV1 {
                constraint: Some(Constraint::NotInSet(schema::IntSet {
                    set: s.iter().cloned().collect(),
                })),
            },
        }
    }

    pub fn proto_int_constraint_to_token_int_constraint(
        input: &schema::IntConstraintV1,
    ) -> Result<IntConstraint, error::Format> {
        use schema::int_constraint_v1::Constraint;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: integer constraint enum is empty".to_string(),
            )),
            Some(Constraint::LessThan(i)) => Ok(IntConstraint::LessThan(*i)),
            Some(Constraint::GreaterThan(i)) => Ok(IntConstraint::GreaterThan(*i)),
            Some(Constraint::LessOrEqual(i)) => Ok(IntConstraint::LessOrEqual(*i)),
            Some(Constraint::GreaterOrEqual(i)) => Ok(IntConstraint::GreaterOrEqual(*i)),
            Some(Constraint::Equal(i)) => Ok(IntConstraint::Equal(*i)),
            Some(Constraint::InSet(schema::IntSet { set })) => {
                Ok(IntConstraint::In(set.iter().cloned().collect()))
            }
            Some(Constraint::NotInSet(schema::IntSet { set })) => {
                Ok(IntConstraint::NotIn(set.iter().cloned().collect()))
            }
        }
    }

    pub fn token_str_constraint_to_proto_str_constraint(
        input: &StrConstraint,
    ) -> schema::StringConstraintV1 {
        use schema::string_constraint_v1::Constraint;

        match input {
            StrConstraint::Prefix(s) => schema::StringConstraintV1 {
                constraint: Some(Constraint::Prefix(s.clone())),
            },
            StrConstraint::Suffix(s) => schema::StringConstraintV1 {
                constraint: Some(Constraint::Suffix(s.clone())),
            },
            StrConstraint::Equal(s) => schema::StringConstraintV1 {
                constraint: Some(Constraint::Equal(s.clone())),
            },
            StrConstraint::Regex(r) => schema::StringConstraintV1 {
                constraint: Some(Constraint::Regex(r.clone())),
            },
            StrConstraint::In(s) => schema::StringConstraintV1 {
                constraint: Some(Constraint::InSet(schema::StringSet {
                    set: s.iter().cloned().collect(),
                })),
            },
            StrConstraint::NotIn(s) => schema::StringConstraintV1 {
                constraint: Some(Constraint::NotInSet(schema::StringSet {
                    set: s.iter().cloned().collect(),
                })),
            },
        }
    }

    pub fn proto_str_constraint_to_token_str_constraint(
        input: &schema::StringConstraintV1,
    ) -> Result<StrConstraint, error::Format> {
        use schema::string_constraint_v1::Constraint;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: string constraint enum is empty".to_string(),
            )),
            Some(Constraint::Prefix(s)) => Ok(StrConstraint::Prefix(s.clone())),
            Some(Constraint::Suffix(s)) => Ok(StrConstraint::Suffix(s.clone())),
            Some(Constraint::Equal(s)) => Ok(StrConstraint::Equal(s.clone())),
            Some(Constraint::InSet(schema::StringSet { set })) => {
                Ok(StrConstraint::In(set.iter().cloned().collect()))
            }
            Some(Constraint::NotInSet(schema::StringSet { set })) => {
                Ok(StrConstraint::NotIn(set.iter().cloned().collect()))
            }
            Some(Constraint::Regex(s)) => Ok(StrConstraint::Regex(s.clone())),
        }
    }

    pub fn token_date_constraint_to_proto_date_constraint(
        input: &DateConstraint,
    ) -> schema::DateConstraintV1 {
        use schema::date_constraint_v1::Constraint;

        match input {
            DateConstraint::Before(i) => schema::DateConstraintV1 {
                constraint: Some(Constraint::Before(*i)),
            },
            DateConstraint::After(i) => schema::DateConstraintV1 {
                constraint: Some(Constraint::After(*i)),
            },
        }
    }

    pub fn proto_date_constraint_to_token_date_constraint(
        input: &schema::DateConstraintV1,
    ) -> Result<DateConstraint, error::Format> {
        use schema::date_constraint_v1::Constraint;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: date constraint enum is empty".to_string(),
            )),
            Some(Constraint::Before(i)) => Ok(DateConstraint::Before(*i)),
            Some(Constraint::After(i)) => Ok(DateConstraint::After(*i)),
        }
    }

    pub fn token_symbol_constraint_to_proto_symbol_constraint(
        input: &SymbolConstraint,
    ) -> schema::SymbolConstraintV1 {
        use schema::symbol_constraint_v1::Constraint;

        match input {
            SymbolConstraint::In(s) => schema::SymbolConstraintV1 {
                constraint: Some(Constraint::InSet(schema::SymbolSet {
                    set: s.iter().cloned().collect(),
                })),
            },
            SymbolConstraint::NotIn(s) => schema::SymbolConstraintV1 {
                constraint: Some(Constraint::NotInSet(schema::SymbolSet {
                    set: s.iter().cloned().collect(),
                })),
            },
        }
    }

    pub fn proto_symbol_constraint_to_token_symbol_constraint(
        input: &schema::SymbolConstraintV1,
    ) -> Result<SymbolConstraint, error::Format> {
        use schema::symbol_constraint_v1::Constraint;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: symbol constraint enum is empty".to_string(),
            )),
            Some(Constraint::InSet(schema::SymbolSet { set })) => {
                Ok(SymbolConstraint::In(set.iter().cloned().collect()))
            }
            Some(Constraint::NotInSet(schema::SymbolSet { set })) => {
                Ok(SymbolConstraint::NotIn(set.iter().cloned().collect()))
            }
        }
    }

    pub fn token_bytes_constraint_to_proto_bytes_constraint(
        input: &BytesConstraint,
    ) -> schema::BytesConstraintV1 {
        use schema::bytes_constraint_v1::Constraint;

        match input {
            BytesConstraint::Equal(s) => schema::BytesConstraintV1 {
                constraint: Some(Constraint::Equal(s.clone())),
            },
            BytesConstraint::In(s) => schema::BytesConstraintV1 {
                constraint: Some(Constraint::InSet(schema::BytesSet {
                    set: s.iter().cloned().collect(),
                })),
            },
            BytesConstraint::NotIn(s) => schema::BytesConstraintV1 {
                constraint: Some(Constraint::NotInSet(schema::BytesSet {
                    set: s.iter().cloned().collect(),
                })),
            },
        }
    }

    pub fn proto_bytes_constraint_to_token_bytes_constraint(
        input: &schema::BytesConstraintV1,
    ) -> Result<BytesConstraint, error::Format> {
        use schema::bytes_constraint_v1::Constraint;

        match &input.constraint {
            None => Err(error::Format::DeserializationError(
                "deserialization error: bytes constraint enum is empty".to_string(),
            )),
            Some(Constraint::Equal(s)) => Ok(BytesConstraint::Equal(s.clone())),
            Some(Constraint::InSet(schema::BytesSet { set })) => {
                Ok(BytesConstraint::In(set.iter().cloned().collect()))
            }
            Some(Constraint::NotInSet(schema::BytesSet { set })) => {
                Ok(BytesConstraint::NotIn(set.iter().cloned().collect()))
            }
        }
    }
}
