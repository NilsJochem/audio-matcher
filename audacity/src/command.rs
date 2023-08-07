pub use NoOut::*;
pub use Out::*;

use crate::RelativeTo;

#[derive(Debug, PartialEq, Clone, derive_more::From)]
pub enum Any<'a> {
    WithOutput(Out<'a>),
    WithoutOutput(NoOut<'a>),
}
impl<'a> ToString for Any<'a> {
    fn to_string(&self) -> String {
        match self {
            Self::WithOutput(x) => x.to_string(),
            Self::WithoutOutput(x) => x.to_string(),
        }
    }
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, command_derive::ToString)]
pub enum Out<'a> {
    /// Used in testing. Sends the Text string back to you.
    Message { text: &'a str },
    /// Gets information in a list in one of three formats.
    // workaround until custom attribute parsing works
    #[allow(non_snake_case)]
    GetInfo {
        Type: InfoTarget,
        format: OutputFormat,
    },
    /// This is an extract from GetInfo Commands, with just one command.
    Help {
        command: Option<&'a str>, // default Help
        format: OutputFormat,
    },
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone, command_derive::ToString)]
pub enum NoOut<'a> {
    /// Creates a new empty mono audio track.
    NewMonoTrack,
    /// Adds an empty stereo track to the project
    NewStereoTrack,
    /// Adds an empty label track to the project
    NewLabelTrack,
    /// Adds an empty time track to the project. Time tracks are used to speed up and slow down audio.
    NewTimeTrack,

    /// Extends the current selection up and/or down into all tracks in the project
    SelAllTracks,
    /// Modifies the temporal selection. Start and End are time. FromEnd allows selection from the end, which is handy to fade in and fade out a track.
    SelectTime {
        start: Option<f64>,
        end: Option<f64>,
        reative_to: Option<RelativeTo>,
    },
    /// Modifies which tracks are selected. First and Last are track numbers. High and Low are for spectral selection. The Mode parameter allows complex selections, e.g adding or removing tracks from the current selection.
    SelectTracks {
        mode: SelectMode,
        track: usize,
    },
    /// Sets properties for a track or channel (or both).Name is used to set the name. It is not used in choosing the track.
    SetTrackStatus {
        name: Option<&'a str>,
        selected: Option<bool>,
        focused: Option<bool>,
    },

    /// Modifies an existing label. You must give it the label number.
    SetLabel {
        nr: usize,
        text: Option<&'a str>,
        start: Option<f64>,
        end: Option<f64>,
        selected: Option<bool>,
    },
    /// Brings up a dialog box showing all of your labels in a keyboard-accessible tabular view. Handy buttons in the dialog let you insert or delete a label, or import and export labels to a file. See Labels Editor for more details.
    EditLabels,
    /// Creates a new, empty label at the cursor or at the selection region.
    AddLabel,
    /// Creates a new, empty label at the current playback or recording position.
    AddLabelPlaying,
    /// Pastes the text on the Audacity clipboard at the cursor position in the currently selected label track. If there is no selection in the label track a point label is created. If a region is selected in the label track a region label is created. If no label track is selected one is created, and a new label is created.
    PasteNewLabel,
    /// When a label track has the yellow focus border, if this option is on, just type to create a label. Otherwise you must create a label first.
    TypeToCreateLabel,

    /// Gets a single preference setting.
    GetPreference {
        name: &'a str,
    },
    /// Sets a single preference setting. Some settings such as them changes require a reload (use Reload=1), but this takes time and slows down a script.
    SetPreference {
        name: &'a str,
        value: &'a str,
        reload: bool,
    },

    /// Creates a new empty project window, to start working on new or imported tracks.
    New,
    /// Presents a standard dialog box where you can select either audio files, a list of files (.LOF) or an Audacity Project file to open.
    Open,
    /// Closes the current project window, prompting you to save your work if you have not saved.
    Close,
    /// Various ways to save a project.
    SaveProject,
    /// Compact your project to save disk space.
    CompactProject,
    /// Opens the standard Page Setup dialog box prior to printing
    PageSetup,
    /// Prints all the waveforms in the current project window (and the contents of Label Tracks or other tracks), with the Timeline above. Everything is printed to one page.
    Print,
    /// Closes all project windows and exits Audacity. If there are any unsaved changes to your project, Audacity will ask if you want to save them.
    Exit,

    /// Zooms in on the horizontal axis of the audio displaying more detail over a shorter length of time.
    ZoomIn,
    /// Zooms to the default view which displays about one inch per second.
    ZoomNormal,
    /// Zooms out displaying less detail over a greater length of time.
    ZoomOut,
    /// Zooms in or out so that the selected audio fills the width of the window.
    ZoomSel,
    /// Changes the zoom back and forth between two preset levels.
    ZoomToggle,
    /// Enable for left-click gestures in the vertical scale to control zooming.
    AdvancedVZoom,

    /// Move backward through currently focused toolbar in Upper Toolbar dock area, Track View and currently focused toolbar in Lower Toolbar dock area. Each use moves the keyboard focus as indicated.
    NextFrame,
    /// Move forward through currently focused toolbar in Upper Toolbar dock area, Track View and currently focused toolbar in Lower Toolbar dock area. Each use moves the keyboard focus as indicated.
    PrevFrame,
    /// Focus one track up
    PrevTrack,
    /// Focus one track down
    NextTrack,
    /// Focus on first track
    FirstTrack,
    /// Focus on last track
    LastTrack,
    /// Focus one track up and select it
    ShiftUp,
    /// Focus one track down and select it
    ShiftDown,
    /// Toggle focus on current track
    Toggle,
    /// Toggle focus on current track
    ToggleAlt,

    Screenshot {
        path: &'a str,
        capture_what: CaptureWhat,
        background: Background,
        to_top: bool,
    },

    PrevWindow,
    NextWindow,

    Delete,
    SplitDelete,
    Duplicate,
    SplitNew,

    ImportLabels,
    ExportLabels,
    ExportMultiple,
    Import2 {
        filename: &'a str,
    },
    Export2 {
        filename: &'a str,
        num_channels: Channels,
    },

    MuteTracks,
    UnmuteTracks,
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum CaptureWhat {
    Window,
    FullWindow,
    WindowPlus,
    Fullscreen,
    Toolbars,
    Effects,
    Scriptables,
    Preferences,
    Selectionbar,
    SpectralSelection,
    Timer,
    Tools,
    Transport,
    Mixer,
    Meter,
    PlayMeter,
    RecordMeter,
    Edit,
    Device,
    Scrub,
    #[display(fmt = "Play-at-Speed")]
    PlayAtSpeed,
    Trackpanel,
    Ruler,
    Tracks,
    FirstTrack,
    FirstTwoTracks,
    FirstThreeTracks,
    FirstFourTracks,
    SecondTrack,
    TracksPlus,
    FirstTrackPlus,
    AllTracks,
    AllTracksPlus,
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum Background {
    Blue,
    White,
    None,
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum InfoTarget {
    Commands,
    Menus,
    Preferences,
    Tracks,
    Clips,
    Envelopes,
    Labels,
    Boxes,
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum OutputFormat {
    #[display(fmt = "JSON")]
    Json,
    Brief,
    #[display(fmt = "LISP")]
    Lisp,
}
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum SelectMode {
    Set,
    Add,
    Remove,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, derive_more::Display)]
pub enum Channels {
    #[display(fmt = "1")]
    Mono,
    #[display(fmt = "2")]
    Stereo,
}

fn push_if_some(s: &mut impl std::fmt::Write, cmd: impl AsRef<str>, param: &Option<impl ToString>) {
    if let Some(value) = param {
        push(s, cmd, value);
    }
}
fn push(s: &mut impl std::fmt::Write, cmd: impl AsRef<str>, value: &impl ToString) {
    write!(s, " {}=", cmd.as_ref()).expect("failed to build command");
    let value = value.to_string();
    if value.contains(' ') {
        write!(s, "\"{value}\"").expect("failed to build escaped command");
    } else {
        write!(s, "{value}").expect("failed to build non-escaped command");
    }
}
