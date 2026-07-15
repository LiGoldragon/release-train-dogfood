//! Pure test: the authored slice-three intent decodes to the six declared
//! language-family members with mainline selectors and their observed main
//! tips as expected bases. This is the flake-check-gated proof that the intent
//! text is well-typed against the library decoder; it reaches no network.
//!
//! The literal below mirrors the canonical intent authored in the primary
//! workspace at `release-trains/language-family-slice-three.nota`.

use synchronizer::release_train::{CandidateSelector, ReleaseTrainIntent};

const SLICE_THREE_INTENT: &str = "(language-family-slice-three [(content-identity Mainline 6cc0408cdb96f174cc8fdf6ca23420038de28450) (name-table Mainline c3237f77c087e6feab49d6cf34971cebc14a11e6) (raw-discovery Mainline a4e8c6df84e6a487ca6fe2f3641f9bafd0b0d8c8) (structural-codec Mainline 104f92454a5ba88b376fa706a9fe38c4a4b65ee0) (core-schema Mainline 33e5be2769b87920b773c7707c1ceb2c97cd42e8) (structural-codec-derive Mainline 348bd89fafefbc13c87b9c5315f7349de38250c6)] [])\n";

#[test]
fn slice_three_intent_decodes_to_six_mainline_members() {
    let intent = ReleaseTrainIntent::from_nota_text(SLICE_THREE_INTENT)
        .expect("authored slice-three intent decodes");
    assert_eq!(intent.name().as_str(), "language-family-slice-three");
    assert_eq!(intent.immutable_externals().len(), 0);

    let expected = [
        (
            "content-identity",
            "6cc0408cdb96f174cc8fdf6ca23420038de28450",
        ),
        ("name-table", "c3237f77c087e6feab49d6cf34971cebc14a11e6"),
        ("raw-discovery", "a4e8c6df84e6a487ca6fe2f3641f9bafd0b0d8c8"),
        (
            "structural-codec",
            "104f92454a5ba88b376fa706a9fe38c4a4b65ee0",
        ),
        ("core-schema", "33e5be2769b87920b773c7707c1ceb2c97cd42e8"),
        (
            "structural-codec-derive",
            "348bd89fafefbc13c87b9c5315f7349de38250c6",
        ),
    ];
    assert_eq!(intent.components().len(), expected.len());
    for (component, (name, base)) in intent.components().iter().zip(expected) {
        assert_eq!(component.component().as_str(), name);
        assert_eq!(component.expected_base().as_str(), base);
        assert!(matches!(component.selector(), CandidateSelector::Mainline));
    }
}
