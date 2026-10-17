//! Dense net stoichiometry matrix extraction and structural network queries.

use crate::model::ValidatedModel;
use std::collections::BTreeMap;

/// Dense net stoichiometry matrix $N$ for a [`ValidatedModel`].
///
/// Rows correspond to floating (non-boundary) species in deterministic model order;
/// columns correspond to reactions in deterministic model order.
#[derive(Clone, Debug, PartialEq)]
pub struct StoichiometryMatrix {
    floating_species: Vec<String>,
    boundary_species: Vec<String>,
    reactions: Vec<String>,
    rows: Vec<Vec<f64>>,
}

impl StoichiometryMatrix {
    /// Extract the net stoichiometry matrix $N$ from a [`ValidatedModel`].
    pub fn from_model(model: &ValidatedModel) -> Self {
        let mut floating_species = Vec::new();
        let mut boundary_species = Vec::new();
        let mut floating_index = BTreeMap::new();

        for sp in model.species() {
            if sp.boundary_condition() {
                boundary_species.push(sp.id().to_owned());
            } else {
                floating_index.insert(sp.id().to_owned(), floating_species.len());
                floating_species.push(sp.id().to_owned());
            }
        }

        let reactions: Vec<String> = model
            .reactions()
            .iter()
            .map(|r| r.id().to_owned())
            .collect();
        let mut rows = vec![vec![0.0; reactions.len()]; floating_species.len()];

        for (col_idx, rxn) in model.reactions().iter().enumerate() {
            for react in rxn.reactants() {
                if let Some(&row_idx) = floating_index.get(react.species()) {
                    rows[row_idx][col_idx] -= react.stoichiometry();
                }
            }
            for prod in rxn.products() {
                if let Some(&row_idx) = floating_index.get(prod.species()) {
                    rows[row_idx][col_idx] += prod.stoichiometry();
                }
            }
        }

        Self {
            floating_species,
            boundary_species,
            reactions,
            rows,
        }
    }

    /// Floating (non-boundary) species identifiers corresponding to matrix rows.
    pub fn floating_species(&self) -> &[String] {
        &self.floating_species
    }

    /// Boundary (`$`) species identifiers omitted from matrix rows.
    pub fn boundary_species(&self) -> &[String] {
        &self.boundary_species
    }

    /// Reaction identifiers corresponding to matrix columns.
    pub fn reactions(&self) -> &[String] {
        &self.reactions
    }

    /// Number of floating-species rows.
    pub fn num_rows(&self) -> usize {
        self.floating_species.len()
    }

    /// Number of reaction columns.
    pub fn num_cols(&self) -> usize {
        self.reactions.len()
    }

    /// Net stoichiometry $N_{i,j}$ for floating species `row` in reaction `col`.
    pub fn get(&self, row: usize, col: usize) -> Option<f64> {
        self.rows.get(row).and_then(|r| r.get(col)).copied()
    }

    /// Net stoichiometry for named floating species and reaction.
    pub fn net_stoichiometry(&self, species_id: &str, reaction_id: &str) -> Option<f64> {
        let row = self.floating_species.iter().position(|s| s == species_id)?;
        let col = self.reactions.iter().position(|r| r == reaction_id)?;
        Some(self.rows[row][col])
    }

    /// Borrow all rows of the dense $M \times R$ stoichiometry matrix.
    pub fn as_rows(&self) -> &[Vec<f64>] {
        &self.rows
    }
}

impl ValidatedModel {
    /// Extract the net stoichiometry matrix $N$ for this validated model.
    pub fn stoichiometry_matrix(&self) -> StoichiometryMatrix {
        StoichiometryMatrix::from_model(self)
    }
}
