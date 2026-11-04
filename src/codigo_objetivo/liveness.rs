//! Análisis de vida de temporales y bloques básicos para `getReg()`.

use std::collections::{BTreeSet, HashMap};

use crate::intermedio::tac::{Base, Instr, Operand, TacFunction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub start: usize,
    pub end: usize,
    pub successors: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Liveness {
    pub blocks: Vec<BasicBlock>,
    pub uses: Vec<BTreeSet<usize>>,
    pub defs: Vec<BTreeSet<usize>>,
    pub live_in: Vec<BTreeSet<usize>>,
    pub live_out: Vec<BTreeSet<usize>>,
    pub live_across_calls: BTreeSet<usize>,
}

impl Liveness {
    pub fn analyze(f: &TacFunction) -> Self {
        let n = f.body.len();
        let labels: HashMap<&str, usize> = f
            .body
            .iter()
            .enumerate()
            .filter_map(|(i, instr)| match instr {
                Instr::Label(label) => Some((label.as_str(), i)),
                _ => None,
            })
            .collect();
        let (uses, defs): (Vec<_>, Vec<_>) = f.body.iter().map(use_def).unzip();
        let successors: Vec<Vec<usize>> = f
            .body
            .iter()
            .enumerate()
            .map(|(i, instr)| instruction_successors(i, instr, n, &labels))
            .collect();

        let mut live_in = vec![BTreeSet::new(); n];
        let mut live_out = vec![BTreeSet::new(); n];
        loop {
            let mut changed = false;
            for i in (0..n).rev() {
                let out: BTreeSet<usize> = successors[i]
                    .iter()
                    .flat_map(|&s| live_in[s].iter().copied())
                    .collect();
                let mut input = uses[i].clone();
                input.extend(out.difference(&defs[i]).copied());
                if out != live_out[i] || input != live_in[i] {
                    live_out[i] = out;
                    live_in[i] = input;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let mut live_across_calls = BTreeSet::new();
        for (i, instr) in f.body.iter().enumerate() {
            if matches!(instr, Instr::Call { .. } | Instr::CallV { .. }) {
                // El resultado nace después del `jal`; solo interesan los
                // valores que existían antes y siguen vivos al regresar.
                live_across_calls.extend(live_out[i].difference(&defs[i]).copied());
            }
        }

        Liveness {
            blocks: basic_blocks(&f.body, &labels),
            uses,
            defs,
            live_in,
            live_out,
            live_across_calls,
        }
    }

    pub fn next_use(&self, temp: usize, after: usize) -> Option<usize> {
        ((after + 1)..self.uses.len()).find(|&i| self.uses[i].contains(&temp))
    }
}

fn temps(op: &Operand, out: &mut BTreeSet<usize>) {
    match op {
        Operand::Temp(k) => {
            out.insert(*k);
        }
        Operand::Var {
            base: Base::Temp(k),
            ..
        }
        | Operand::Mem {
            base: Base::Temp(k),
            ..
        } => {
            out.insert(*k);
        }
        _ => {}
    }
}

fn define(dst: &Operand, uses: &mut BTreeSet<usize>, defs: &mut BTreeSet<usize>) {
    if let Operand::Temp(k) = dst {
        defs.insert(*k);
    } else {
        temps(dst, uses);
    }
}

fn use_def(instr: &Instr) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let mut uses = BTreeSet::new();
    let mut defs = BTreeSet::new();
    match instr {
        Instr::Copy { dst, src } | Instr::Unary { dst, src, .. } => {
            temps(src, &mut uses);
            define(dst, &mut uses, &mut defs);
        }
        Instr::Binary {
            dst, left, right, ..
        } => {
            temps(left, &mut uses);
            temps(right, &mut uses);
            define(dst, &mut uses, &mut defs);
        }
        Instr::IfTrue { cond, .. } | Instr::IfFalse { cond, .. } => temps(cond, &mut uses),
        Instr::IfRel { left, right, .. } => {
            temps(left, &mut uses);
            temps(right, &mut uses);
        }
        Instr::Param(op)
        | Instr::Link(op)
        | Instr::Print { value: op, .. }
        | Instr::CheckZero(op)
        | Instr::Return(Some(op)) => {
            temps(op, &mut uses);
        }
        Instr::Call { dst, .. } => {
            if let Some(dst) = dst {
                define(dst, &mut uses, &mut defs);
            }
        }
        Instr::CallV { dst, target, .. } => {
            temps(target, &mut uses);
            if let Some(dst) = dst {
                define(dst, &mut uses, &mut defs);
            }
        }
        Instr::Load { dst, base, index } => {
            temps(base, &mut uses);
            temps(index, &mut uses);
            define(dst, &mut uses, &mut defs);
        }
        Instr::Store { base, index, src } => {
            temps(base, &mut uses);
            temps(index, &mut uses);
            temps(src, &mut uses);
        }
        Instr::Alloc { dst, size } => {
            temps(size, &mut uses);
            define(dst, &mut uses, &mut defs);
        }
        Instr::AddrOf { dst, .. } | Instr::GetException(dst) => define(dst, &mut uses, &mut defs),
        Instr::CheckBounds { array, index } => {
            temps(array, &mut uses);
            temps(index, &mut uses);
        }
        Instr::Label(_)
        | Instr::Goto(_)
        | Instr::Return(None)
        | Instr::TryBegin(_)
        | Instr::TryEnd => {}
    }
    (uses, defs)
}

fn instruction_successors(
    i: usize,
    instr: &Instr,
    n: usize,
    labels: &HashMap<&str, usize>,
) -> Vec<usize> {
    let next = || (i + 1 < n).then_some(i + 1);
    match instr {
        Instr::Goto(label) => labels.get(label.as_str()).copied().into_iter().collect(),
        Instr::IfTrue { label, .. } | Instr::IfFalse { label, .. } | Instr::IfRel { label, .. } => {
            let mut result: Vec<usize> = labels.get(label.as_str()).copied().into_iter().collect();
            if let Some(next) = next() {
                result.push(next);
            }
            result.sort_unstable();
            result.dedup();
            result
        }
        Instr::Return(_) => Vec::new(),
        _ => next().into_iter().collect(),
    }
}

fn basic_blocks(body: &[Instr], labels: &HashMap<&str, usize>) -> Vec<BasicBlock> {
    if body.is_empty() {
        return Vec::new();
    }
    let mut leaders = BTreeSet::from([0usize]);
    for (i, instr) in body.iter().enumerate() {
        if matches!(instr, Instr::Label(_)) {
            leaders.insert(i);
        }
        if matches!(
            instr,
            Instr::Goto(_)
                | Instr::IfTrue { .. }
                | Instr::IfFalse { .. }
                | Instr::IfRel { .. }
                | Instr::Return(_)
                | Instr::Call { .. }
                | Instr::CallV { .. }
        ) && i + 1 < body.len()
        {
            leaders.insert(i + 1);
        }
        match instr {
            Instr::Goto(label)
            | Instr::IfTrue { label, .. }
            | Instr::IfFalse { label, .. }
            | Instr::IfRel { label, .. } => {
                if let Some(&target) = labels.get(label.as_str()) {
                    leaders.insert(target);
                }
            }
            _ => {}
        }
    }
    let starts: Vec<usize> = leaders.into_iter().collect();
    let index_of: HashMap<usize, usize> = starts.iter().enumerate().map(|(b, &i)| (i, b)).collect();
    starts
        .iter()
        .enumerate()
        .map(|(b, &start)| {
            let end = starts.get(b + 1).copied().unwrap_or(body.len()) - 1;
            let instruction_succ = instruction_successors(end, &body[end], body.len(), labels);
            let mut successors = Vec::new();
            for target in instruction_succ {
                let block_start = starts
                    .iter()
                    .copied()
                    .take_while(|&s| s <= target)
                    .last()
                    .unwrap();
                let block = index_of[&block_start];
                if !successors.contains(&block) {
                    successors.push(block);
                }
            }
            BasicBlock {
                start,
                end,
                successors,
            }
        })
        .collect()
}
