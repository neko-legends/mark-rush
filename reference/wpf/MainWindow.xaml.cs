using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Navigation;
using System.Windows.Threading;

namespace MarkRush;

public partial class MainWindow : Window
{
    private const string LoadingMarker = "__MARKRUSH_LOADING__";
    private const int StreamBufferSize = 1024 * 128;
    private const int MaxStoredFileViewStates = 250;
    private const double UnwrappedDocumentWidth = 12000;
    private const double MiddleScrollDeadZone = 8.0;
    private const int WmSizing = 0x0214;
    private const int WmEnterSizeMove = 0x0231;
    private const int WmExitSizeMove = 0x0232;

    private static readonly HashSet<string> MarkdownExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".md",
        ".markdown",
        ".mdown",
        ".mdx",
        ".mkd",
        ".rmd"
    };

    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    private static readonly AppTheme[] Themes =
    {
        new(
            "EVA Dark",
            "#1E1F22",
            "#17181B",
            "#1E1F22",
            "#4E5058",
            "#DBDEE1",
            "#949BA4",
            "#232428",
            "#5865F2",
            "#313338",
            "#383A40",
            "#4149B8",
            "#4E5058",
            "#5865F2",
            "#F2A65A",
            "#050505",
            "#232428"),
        new(
            "Pearl White",
            "#F8F8FC",
            "#ECECF4",
            "#F8F8FC",
            "#D0D0DC",
            "#1E1C2D",
            "#72728A",
            "#FFFFFF",
            "#635BDC",
            "#F0F0F6",
            "#E4E4EC",
            "#D8D7F4",
            "#D0D0DC",
            "#BFBDF5",
            "#F2A65A",
            "#1E1C2D",
            "#F0F0F6"),
        new(
            "Abyss Teal",
            "#101415",
            "#0B1011",
            "#101415",
            "#3A5257",
            "#D9EEEE",
            "#93A5A8",
            "#161D1F",
            "#2EC4B6",
            "#1F2A2C",
            "#28373A",
            "#286C68",
            "#3A5257",
            "#246D67",
            "#F28F6F",
            "#101415",
            "#161D1F"),
        new(
            "Ember Glow",
            "#18130F",
            "#120F0C",
            "#18130F",
            "#604D3E",
            "#F1E7DC",
            "#A4968B",
            "#241E18",
            "#F2A65A",
            "#352C24",
            "#45392E",
            "#755633",
            "#604D3E",
            "#8C633A",
            "#F2A65A",
            "#050505",
            "#241E18"),
        new(
            "Mosswood",
            "#101511",
            "#0B100D",
            "#101511",
            "#3E5D4C",
            "#E2ECE6",
            "#9AA89F",
            "#18201B",
            "#4CC38A",
            "#233028",
            "#2C3D33",
            "#315B43",
            "#3E5D4C",
            "#36724E",
            "#C69CF2",
            "#101511",
            "#18201B"),
        new(
            "Rose Noir",
            "#140F10",
            "#0E0A0B",
            "#140F10",
            "#5B4149",
            "#F0E3E8",
            "#A19398",
            "#21191B",
            "#E07383",
            "#322629",
            "#422F34",
            "#713946",
            "#5B4149",
            "#86404F",
            "#F2A65A",
            "#050505",
            "#21191B"),
        new(
            "High Contrast",
            "#000000",
            "#080808",
            "#000000",
            "#777777",
            "#FFFFFF",
            "#CFCFCF",
            "#000000",
            "#00E5FF",
            "#101010",
            "#1C1C1C",
            "#003F47",
            "#999999",
            "#0078D7",
            "#FFFF00",
            "#000000",
            "#151515")
    };

    private readonly List<int> _matchOffsets = new();
    private readonly DispatcherTimer _middleScrollTimer;
    private readonly Dictionary<string, Paragraph> _headingAnchors = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, int> _headingAnchorCounts = new(StringComparer.OrdinalIgnoreCase);
    private AppSettings _settings = new();
    private string? _rootPath;
    private string? _currentFilePath;
    private NodeInfo? _treeContextNode;
    private string _lastSavedText = string.Empty;
    private string _renderedText = string.Empty;
    private bool _loadingEditorText;
    private bool _initializing;
    private bool _suppressTreeSelection;
    private bool _isDirty;
    private bool _middleScrolling;
    private bool _inSizeMoveLoop;
    private bool _deferDocumentDuringResize;
    private bool _resizeHadViewerVisible;
    private bool _resizeHadEditorVisible;
    private bool _renderedSearchHighlightActive;
    private Point _middleScrollStart;
    private ScrollViewer? _middleScrollViewer;
    private HwndSource? _windowSource;
    private int _currentMatchIndex = -1;
    private int _loadVersion;

    private bool IsEditing => EditModeBox.IsChecked == true;

    public MainWindow()
    {
        InitializeComponent();
        _middleScrollTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
        _middleScrollTimer.Tick += MiddleScrollTimer_Tick;
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        _windowSource = (HwndSource?)PresentationSource.FromVisual(this);
        _windowSource?.AddHook(WindowMessageHook);
    }

    private IntPtr WindowMessageHook(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        switch (msg)
        {
            case WmSizing:
                if (_inSizeMoveLoop)
                {
                    BeginDeferredResize();
                }

                break;
            case WmEnterSizeMove:
                _inSizeMoveLoop = true;
                break;
            case WmExitSizeMove:
                _inSizeMoveLoop = false;
                EndDeferredResize();
                break;
        }

        return IntPtr.Zero;
    }

    private void Window_Loaded(object sender, RoutedEventArgs e)
    {
        _initializing = true;
        _settings = LoadSettings();
        ThemeBox.ItemsSource = Themes.Select(t => t.Name).ToArray();
        var theme = FindTheme(_settings.ThemeName);
        ThemeBox.SelectedItem = theme.Name;
        ApplyTheme(theme);
        ApplyWindowSettings();
        WordWrapBox.IsChecked = _settings.WordWrap;
        ApplyWordWrap(_settings.WordWrap);
        UpdateEditorMode();
        _initializing = false;

        var startupTarget = ResolveStartupTarget();
        SetRootDirectory(startupTarget.RootPath);

        if (startupTarget.FilePath is not null)
        {
            _ = LoadFileAsync(startupTarget.FilePath);
        }
        else
        {
            Editor.IsEnabled = false;
            Viewer.IsEnabled = false;
            UpdateCommandState();
            UpdateWindowTitle();
        }
    }

    private void Window_Closing(object? sender, CancelEventArgs e)
    {
        if (!PromptSaveIfDirty())
        {
            e.Cancel = true;
            return;
        }

        RememberCurrentViewState();
        SaveSettings();
        _windowSource?.RemoveHook(WindowMessageHook);
    }

    private void Window_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (Keyboard.Modifiers == ModifierKeys.Control && e.Key == Key.F)
        {
            FocusSearch();
            e.Handled = true;
            return;
        }

        if (Keyboard.Modifiers == ModifierKeys.Control && e.Key == Key.S)
        {
            SaveCurrentFile();
            e.Handled = true;
            return;
        }

        if (Keyboard.Modifiers == ModifierKeys.Control &&
            IsDocumentSurfaceFocused() &&
            (e.Key == Key.V || e.Key == Key.X))
        {
            var pasteAtEnd = Viewer.IsKeyboardFocusWithin;
            if (!EnsureEditorCanModify())
            {
                return;
            }

            if (pasteAtEnd)
            {
                Editor.CaretIndex = Editor.Text.Length;
            }

            if (e.Key == Key.V)
            {
                Editor.Paste();
            }
            else
            {
                Editor.Cut();
            }

            e.Handled = true;
            return;
        }

        if (e.Key == Key.F3)
        {
            MoveToMatch(Keyboard.Modifiers == ModifierKeys.Shift ? -1 : 1);
            e.Handled = true;
            return;
        }

        if (e.Key == Key.F5)
        {
            RefreshFolder();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Escape && SearchBox.IsKeyboardFocusWithin)
        {
            FocusDocumentSurface();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Escape && _middleScrolling)
        {
            StopMiddleScroll();
            e.Handled = true;
        }
    }

    private void DocumentSurface_PreviewMouseDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Middle || sender is not UIElement surface)
        {
            return;
        }

        var scrollViewer = FindVisualChild<ScrollViewer>(surface);
        if (scrollViewer is null)
        {
            return;
        }

        _middleScrollViewer = scrollViewer;
        _middleScrollStart = e.GetPosition(this);
        _middleScrolling = true;
        surface.CaptureMouse();
        Mouse.OverrideCursor = Cursors.ScrollNS;
        _middleScrollTimer.Start();
        e.Handled = true;
    }

    private void DocumentSurface_PreviewMouseUp(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton == MouseButton.Middle)
        {
            StopMiddleScroll();
            e.Handled = true;
        }
    }

    private void DocumentSurface_LostMouseCapture(object sender, MouseEventArgs e)
    {
        if (_middleScrolling)
        {
            StopMiddleScroll();
        }
    }

    private void MiddleScrollTimer_Tick(object? sender, EventArgs e)
    {
        if (!_middleScrolling || _middleScrollViewer is null)
        {
            StopMiddleScroll();
            return;
        }

        var current = Mouse.GetPosition(this);
        var delta = current.Y - _middleScrollStart.Y;
        var magnitude = Math.Abs(delta);
        if (magnitude <= MiddleScrollDeadZone)
        {
            return;
        }

        var direction = Math.Sign(delta);
        var speed = Math.Min(80, Math.Pow((magnitude - MiddleScrollDeadZone) / 10.0, 1.2) * 4.0);
        _middleScrollViewer.ScrollToVerticalOffset(_middleScrollViewer.VerticalOffset + (direction * speed));
    }

    private void StopMiddleScroll()
    {
        _middleScrolling = false;
        _middleScrollTimer.Stop();
        _middleScrollViewer = null;
        Mouse.OverrideCursor = null;

        if (Viewer.IsMouseCaptured)
        {
            Viewer.ReleaseMouseCapture();
        }

        if (Editor.IsMouseCaptured)
        {
            Editor.ReleaseMouseCapture();
        }
    }

    private void BeginDeferredResize()
    {
        if (_deferDocumentDuringResize)
        {
            return;
        }

        StopMiddleScroll();
        _resizeHadViewerVisible = Viewer.Visibility == Visibility.Visible;
        _resizeHadEditorVisible = Editor.Visibility == Visibility.Visible;
        Viewer.Visibility = Visibility.Collapsed;
        Editor.Visibility = Visibility.Collapsed;
        ResizePlaceholder.Visibility = Visibility.Visible;
        _deferDocumentDuringResize = true;
    }

    private void EndDeferredResize()
    {
        if (!_deferDocumentDuringResize)
        {
            return;
        }

        ResizePlaceholder.Visibility = Visibility.Collapsed;
        Viewer.Visibility = _resizeHadViewerVisible ? Visibility.Visible : Visibility.Collapsed;
        Editor.Visibility = _resizeHadEditorVisible ? Visibility.Visible : Visibility.Collapsed;
        _deferDocumentDuringResize = false;

        Dispatcher.BeginInvoke(
            () =>
            {
                if (IsEditing && Editor.Visibility == Visibility.Visible)
                {
                    Editor.Focus();
                }
                else if (Viewer.Visibility == Visibility.Visible)
                {
                    Viewer.Focus();
                }
            },
            DispatcherPriority.Background);
    }

    private void OpenFolder_Click(object sender, RoutedEventArgs e)
    {
        RememberCurrentViewState();

        if (!PromptSaveIfDirty())
        {
            return;
        }

        var dialog = new Microsoft.Win32.OpenFolderDialog
        {
            Title = "Choose a folder with Markdown files",
            InitialDirectory = Directory.Exists(_rootPath)
                ? _rootPath
                : Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments)
        };

        if (dialog.ShowDialog(this) == true && Directory.Exists(dialog.FolderName))
        {
            SetRootDirectory(dialog.FolderName);
            ClearCurrentFile();
            SaveSettings();
        }
    }

    private void RefreshFolder_Click(object sender, RoutedEventArgs e)
    {
        RefreshFolder();
    }

    private void Reload_Click(object sender, RoutedEventArgs e)
    {
        RememberCurrentViewState();

        if (_currentFilePath is null || !PromptSaveIfDirty())
        {
            return;
        }

        _ = LoadFileAsync(_currentFilePath);
    }

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        SaveCurrentFile();
    }

    private void EditModeBox_Changed(object sender, RoutedEventArgs e)
    {
        UpdateEditorMode();
        UpdateCommandState();
    }

    private void WordWrapBox_Changed(object sender, RoutedEventArgs e)
    {
        var enabled = WordWrapBox.IsChecked == true;
        var anchor = _currentFilePath is null
            ? null
            : IsEditing
                ? CaptureEditorAnchor()
                : CaptureRenderedAnchor();
        ApplyWordWrap(enabled);

        if (!_initializing)
        {
            if (IsEditing)
            {
                RestoreEditorAnchor(anchor);
            }
            else
            {
                RestoreRenderedAnchor(anchor);
            }
        }

        if (_initializing)
        {
            return;
        }

        _settings.WordWrap = enabled;
        SaveSettings();
    }

    private void ThemeBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (ThemeBox.SelectedItem is not string themeName)
        {
            return;
        }

        var theme = FindTheme(themeName);
        ApplyTheme(theme);
        _settings.ThemeName = theme.Name;

        if (!_initializing)
        {
            RememberCurrentViewState();
            RenderMarkdown(Editor.Text);
            RestoreCurrentViewState();
            SaveSettings();
        }
    }

    private void FileTree_SelectedItemChanged(object sender, RoutedPropertyChangedEventArgs<object> e)
    {
        if (_suppressTreeSelection)
        {
            return;
        }

        if (FileTree.SelectedItem is not TreeViewItem item ||
            item.Tag is not NodeInfo { IsDirectory: false } node)
        {
            return;
        }

        RememberCurrentViewState();

        if (!PromptSaveIfDirty())
        {
            return;
        }

        _ = LoadFileAsync(node.Path);
    }

    private void FileTreeItem_Expanded(object sender, RoutedEventArgs e)
    {
        if (e.OriginalSource is TreeViewItem item)
        {
            LoadDirectoryChildren(item);
        }
    }

    private void FileTree_PreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
    {
        _treeContextNode = e.OriginalSource is DependencyObject source
            ? FindVisualParent<TreeViewItem>(source)?.Tag as NodeInfo
            : null;
    }

    private void FileTreeContextMenu_Opened(object sender, RoutedEventArgs e)
    {
        var node = GetTreeContextNode();
        ShowInExplorerMenuItem.IsEnabled = node is not null &&
                                           (node.IsDirectory ? Directory.Exists(node.Path) : File.Exists(node.Path));
    }

    private void ShowInExplorer_Click(object sender, RoutedEventArgs e)
    {
        var node = GetTreeContextNode();
        if (node is null)
        {
            return;
        }

        ShowInFileExplorer(node);
    }

    private void SearchBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        RebuildSearchIndex();
        UpdateCommandState();
    }

    private void SearchBox_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            MoveToMatch(Keyboard.Modifiers == ModifierKeys.Shift ? -1 : 1);
            e.Handled = true;
        }
    }

    private void PreviousMatch_Click(object sender, RoutedEventArgs e)
    {
        MoveToMatch(-1);
    }

    private void NextMatch_Click(object sender, RoutedEventArgs e)
    {
        MoveToMatch(1);
    }

    private void CopyFilePathButton_Click(object sender, RoutedEventArgs e)
    {
        if (_currentFilePath is null)
        {
            return;
        }

        Clipboard.SetText(_currentFilePath);
        StatusText.Text = "Copied file path";
    }

    private void CutMenuItem_Click(object sender, RoutedEventArgs e)
    {
        if (Viewer.IsKeyboardFocusWithin)
        {
            Editor.CaretIndex = Editor.Text.Length;
        }

        if (EnsureEditorCanModify())
        {
            Editor.Cut();
        }
    }

    private void CopyMenuItem_Click(object sender, RoutedEventArgs e)
    {
        if (Editor.Visibility == Visibility.Visible)
        {
            Editor.Copy();
        }
        else
        {
            Viewer.Copy();
        }
    }

    private void PasteMenuItem_Click(object sender, RoutedEventArgs e)
    {
        if (Viewer.IsKeyboardFocusWithin || Viewer.Visibility == Visibility.Visible)
        {
            Editor.CaretIndex = Editor.Text.Length;
        }

        if (EnsureEditorCanModify())
        {
            Editor.Paste();
        }
    }

    private void SelectAllMenuItem_Click(object sender, RoutedEventArgs e)
    {
        if (Editor.Visibility == Visibility.Visible)
        {
            Editor.SelectAll();
        }
        else
        {
            Viewer.SelectAll();
        }
    }

    private void Editor_TextChanged(object sender, TextChangedEventArgs e)
    {
        if (!_loadingEditorText && _currentFilePath is not null)
        {
            _isDirty = !string.Equals(Editor.Text, _lastSavedText, StringComparison.Ordinal);
            UpdateWindowTitle();
            UpdateCommandState();
        }

        UpdateStats();

        if (!string.IsNullOrEmpty(SearchBox.Text))
        {
            RebuildSearchIndex();
            UpdateCommandState();
        }
    }

    private void RefreshFolder()
    {
        if (_rootPath is null || !Directory.Exists(_rootPath))
        {
            return;
        }

        RememberCurrentViewState();

        if (!PromptSaveIfDirty())
        {
            return;
        }

        var currentFile = _currentFilePath;
        SetRootDirectory(_rootPath);

        if (currentFile is not null && File.Exists(currentFile))
        {
            _ = LoadFileAsync(currentFile);
        }
    }

    private void SetRootDirectory(string path)
    {
        _rootPath = path;
        _settings.LastRootPath = path;
        RootPathText.Text = path;
        FileTree.Items.Clear();

        var root = CreateDirectoryItem(new DirectoryInfo(path), true);
        FileTree.Items.Add(root);
        root.IsExpanded = true;
        LoadDirectoryChildren(root);

        RefreshFolderButton.IsEnabled = true;
        StatusText.Text = "Folder loaded";
    }

    private TreeViewItem CreateDirectoryItem(DirectoryInfo directory, bool isRoot = false)
    {
        var item = new TreeViewItem
        {
            Header = isRoot ? directory.FullName : directory.Name,
            Tag = new NodeInfo(directory.FullName, true),
            ToolTip = directory.FullName
        };

        item.Items.Add(LoadingMarker);
        return item;
    }

    private static TreeViewItem CreateFileItem(FileInfo file)
    {
        return new TreeViewItem
        {
            Header = file.Name,
            Tag = new NodeInfo(file.FullName, false),
            ToolTip = file.FullName
        };
    }

    private void LoadDirectoryChildren(TreeViewItem item)
    {
        if (item.Tag is not NodeInfo { IsDirectory: true } node || !HasLoadingMarker(item))
        {
            return;
        }

        item.Items.Clear();

        try
        {
            var directory = new DirectoryInfo(node.Path);

            foreach (var childDirectory in directory.EnumerateDirectories()
                         .Where(IsVisibleDirectory)
                         .OrderBy(d => d.Name, StringComparer.CurrentCultureIgnoreCase))
            {
                item.Items.Add(CreateDirectoryItem(childDirectory));
            }

            foreach (var markdownFile in directory.EnumerateFiles()
                         .Where(IsMarkdownFile)
                         .OrderBy(f => f.Name, StringComparer.CurrentCultureIgnoreCase))
            {
                item.Items.Add(CreateFileItem(markdownFile));
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            item.Items.Add(new TreeViewItem
            {
                Header = "Unable to read folder",
                IsEnabled = false,
                Foreground = System.Windows.Media.Brushes.Gray
            });
        }
    }

    private async Task LoadFileAsync(string path)
    {
        var loadVersion = ++_loadVersion;

        try
        {
            StatusText.Text = "Loading file...";
            _loadingEditorText = true;

            var text = await ReadFileTextAsync(path);
            if (loadVersion != _loadVersion)
            {
                return;
            }

            Editor.Text = text;
            RenderMarkdown(text);
            Editor.IsEnabled = true;
            Viewer.IsEnabled = true;

            _currentFilePath = path;
            _settings.LastFilePath = path;
            _lastSavedText = text;
            _isDirty = false;

            FileNameText.Text = Path.GetFileName(path);
            FilePathText.Text = path;

            RebuildSearchIndex();
            UpdateStats();
            UpdateWindowTitle();
            UpdateCommandState();
            RevealFileInTree(path);
            RestoreCurrentViewState();
            SaveSettings();
            StatusText.Text = "File loaded";
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            MessageBox.Show(this, ex.Message, "MarkRush", MessageBoxButton.OK, MessageBoxImage.Error);
            StatusText.Text = "Could not load file";
        }
        catch (Exception ex)
        {
            App.LogCrash(ex);
            MessageBox.Show(this, "MarkRush could not load this file. Details were written to MarkRush.crash.log.", "MarkRush", MessageBoxButton.OK, MessageBoxImage.Error);
            StatusText.Text = "Could not load file";
        }
        finally
        {
            if (loadVersion == _loadVersion)
            {
                _loadingEditorText = false;
            }
        }
    }

    private bool SaveCurrentFile()
    {
        if (_currentFilePath is null)
        {
            return true;
        }

        try
        {
            using var stream = new FileStream(
                _currentFilePath,
                FileMode.Create,
                FileAccess.Write,
                FileShare.Read,
                StreamBufferSize,
                FileOptions.SequentialScan);
            using var writer = new StreamWriter(stream, new UTF8Encoding(false));
            writer.Write(Editor.Text);

            _lastSavedText = Editor.Text;
            _isDirty = false;
            RenderMarkdown(Editor.Text);
            UpdateWindowTitle();
            UpdateCommandState();
            StatusText.Text = "Saved";
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            MessageBox.Show(this, ex.Message, "MarkRush", MessageBoxButton.OK, MessageBoxImage.Error);
            StatusText.Text = "Could not save file";
            return false;
        }
    }

    private bool PromptSaveIfDirty()
    {
        if (!_isDirty)
        {
            return true;
        }

        var fileName = Path.GetFileName(_currentFilePath) ?? "current file";
        var result = MessageBox.Show(
            this,
            $"Save changes to {fileName}?",
            "MarkRush",
            MessageBoxButton.YesNoCancel,
            MessageBoxImage.Question);

        return result switch
        {
            MessageBoxResult.Yes => SaveCurrentFile(),
            MessageBoxResult.No => true,
            _ => false
        };
    }

    private void FocusSearch()
    {
        if (!SearchBox.IsEnabled)
        {
            return;
        }

        var selectedText = GetSelectedTextForSearch();
        if (!string.IsNullOrWhiteSpace(selectedText))
        {
            SearchBox.Text = selectedText;
        }

        SearchBox.Focus();
        SearchBox.SelectAll();
    }

    private string GetSelectedTextForSearch()
    {
        if (Editor.Visibility == Visibility.Visible && Editor.SelectionLength > 0)
        {
            return NormalizeSearchSelection(Editor.SelectedText);
        }

        if (Viewer.Visibility == Visibility.Visible && !Viewer.Selection.IsEmpty)
        {
            return NormalizeSearchSelection(new TextRange(Viewer.Selection.Start, Viewer.Selection.End).Text);
        }

        return string.Empty;
    }

    private static string NormalizeSearchSelection(string text)
    {
        return text
            .Replace("\r\n", " ", StringComparison.Ordinal)
            .Replace('\r', ' ')
            .Replace('\n', ' ')
            .Trim();
    }

    private void RebuildSearchIndex()
    {
        ClearRenderedSearchHighlight();
        _matchOffsets.Clear();
        _currentMatchIndex = -1;

        var needle = SearchBox.Text;
        if (string.IsNullOrEmpty(needle))
        {
            SearchStatusText.Text = _currentFilePath is null ? "No file" : "No search";
            return;
        }

        var haystack = IsEditing ? Editor.Text : _renderedText;
        var offset = 0;
        while (offset < haystack.Length)
        {
            var match = haystack.IndexOf(needle, offset, StringComparison.OrdinalIgnoreCase);
            if (match < 0)
            {
                break;
            }

            _matchOffsets.Add(match);
            offset = match + Math.Max(needle.Length, 1);
        }

        SearchStatusText.Text = _matchOffsets.Count == 0
            ? "No matches"
            : $"{_matchOffsets.Count:N0} match{(_matchOffsets.Count == 1 ? string.Empty : "es")}";
    }

    private void MoveToMatch(int direction)
    {
        if (string.IsNullOrEmpty(SearchBox.Text))
        {
            FocusSearch();
            return;
        }

        if (_matchOffsets.Count == 0)
        {
            RebuildSearchIndex();
        }

        if (_matchOffsets.Count == 0)
        {
            SearchStatusText.Text = "No matches";
            return;
        }

        if (_currentMatchIndex < 0)
        {
            _currentMatchIndex = FindInitialMatchIndex(direction);
        }
        else
        {
            _currentMatchIndex = (_currentMatchIndex + direction + _matchOffsets.Count) % _matchOffsets.Count;
        }

        var offset = _matchOffsets[_currentMatchIndex];
        if (IsEditing)
        {
            Editor.Focus();
            Editor.Select(offset, SearchBox.Text.Length);
            Editor.ScrollToLine(Editor.GetLineIndexFromCharacterIndex(offset));
        }
        else
        {
            HighlightRenderedText(offset, SearchBox.Text.Length);
        }

        SearchStatusText.Text = $"{_currentMatchIndex + 1:N0} of {_matchOffsets.Count:N0}";
    }

    private int FindInitialMatchIndex(int direction)
    {
        var caret = IsEditing ? Editor.CaretIndex : GetRenderedSelectionOffset();
        if (direction < 0)
        {
            for (var i = _matchOffsets.Count - 1; i >= 0; i--)
            {
                if (_matchOffsets[i] < caret)
                {
                    return i;
                }
            }

            return _matchOffsets.Count - 1;
        }

        for (var i = 0; i < _matchOffsets.Count; i++)
        {
            if (_matchOffsets[i] >= caret)
            {
                return i;
            }
        }

        return 0;
    }

    private void ClearCurrentFile()
    {
        RememberCurrentViewState();
        ++_loadVersion;
        _currentFilePath = null;
        _lastSavedText = string.Empty;
        _isDirty = false;
        _loadingEditorText = true;
        Editor.Text = string.Empty;
        Editor.IsEnabled = false;
        Viewer.Document = CreateEmptyDocument();
        Viewer.IsEnabled = false;
        _renderedText = string.Empty;
        _loadingEditorText = false;
        FileNameText.Text = "No file selected";
        FilePathText.Text = string.Empty;
        SearchBox.Text = string.Empty;
        _settings.LastFilePath = null;
        UpdateStats();
        UpdateWindowTitle();
        UpdateCommandState();
    }

    private void ApplyWordWrap(bool enabled)
    {
        Editor.TextWrapping = enabled ? TextWrapping.Wrap : TextWrapping.NoWrap;
        Editor.HorizontalScrollBarVisibility = enabled
            ? ScrollBarVisibility.Disabled
            : ScrollBarVisibility.Auto;
        Viewer.HorizontalScrollBarVisibility = enabled
            ? ScrollBarVisibility.Disabled
            : ScrollBarVisibility.Auto;

        ApplyDocumentWordWrap(Viewer.Document, enabled);
    }

    private static void ApplyDocumentWordWrap(FlowDocument document, bool enabled)
    {
        if (enabled)
        {
            document.ClearValue(FlowDocument.PageWidthProperty);
            document.ClearValue(FlowDocument.MinPageWidthProperty);
            document.ClearValue(FlowDocument.MaxPageWidthProperty);
            document.ClearValue(FlowDocument.ColumnWidthProperty);
            return;
        }

        document.PageWidth = UnwrappedDocumentWidth;
        document.MinPageWidth = UnwrappedDocumentWidth;
        document.MaxPageWidth = UnwrappedDocumentWidth;
        document.ColumnWidth = UnwrappedDocumentWidth;
    }

    private void RenderMarkdown(string markdown)
    {
        try
        {
            var document = BuildMarkdownDocument(markdown);
            Viewer.Document = document;
            _renderedText = ExtractTextRuns(document);
            _renderedSearchHighlightActive = false;
        }
        catch (Exception ex)
        {
            App.LogCrash(ex);
            var document = CreateEmptyDocument();
            document.Blocks.Clear();
            AddCodeBlock(document, markdown);
            Viewer.Document = document;
            _renderedText = markdown;
            _renderedSearchHighlightActive = false;
            StatusText.Text = "Rendered fallback";
        }
    }

    private FlowDocument BuildMarkdownDocument(string markdown)
    {
        var document = CreateEmptyDocument();
        document.Blocks.Clear();
        _headingAnchors.Clear();
        _headingAnchorCounts.Clear();

        var normalized = markdown.Replace("\r\n", "\n").Replace('\r', '\n');
        var lines = normalized.Split('\n');
        var paragraph = new StringBuilder();
        var code = new StringBuilder();
        var inCodeFence = false;
        var fence = string.Empty;

        void FlushParagraph()
        {
            if (paragraph.Length == 0)
            {
                return;
            }

            AddParagraph(document, paragraph.ToString().Trim());
            paragraph.Clear();
        }

        for (var lineIndex = 0; lineIndex < lines.Length; lineIndex++)
        {
            var line = lines[lineIndex];
            var trimmed = line.Trim();

            if (inCodeFence)
            {
                if (trimmed.StartsWith(fence, StringComparison.Ordinal))
                {
                    AddCodeBlock(document, code.ToString().TrimEnd('\n'));
                    code.Clear();
                    inCodeFence = false;
                    fence = string.Empty;
                }
                else
                {
                    code.AppendLine(line);
                }

                continue;
            }

            if (IsCodeFence(trimmed, out var detectedFence))
            {
                FlushParagraph();
                inCodeFence = true;
                fence = detectedFence;
                continue;
            }

            if (string.IsNullOrWhiteSpace(line))
            {
                FlushParagraph();
                continue;
            }

            if (TryReadTable(lines, lineIndex, out var table, out var consumedLines))
            {
                FlushParagraph();
                AddTable(document, table);
                lineIndex += consumedLines - 1;
                continue;
            }

            if (TryGetHeading(line, out var level, out var headingText))
            {
                FlushParagraph();
                AddHeading(document, level, headingText);
                continue;
            }

            if (IsHorizontalRule(trimmed))
            {
                FlushParagraph();
                AddHorizontalRule(document);
                continue;
            }

            if (TryGetBlockquote(line, out var quoteText))
            {
                FlushParagraph();
                AddBlockquote(document, quoteText);
                continue;
            }

            if (TryGetListItem(line, out var marker, out var listText))
            {
                FlushParagraph();
                AddListItem(document, marker, listText);
                continue;
            }

            if (paragraph.Length > 0)
            {
                paragraph.Append(' ');
            }

            paragraph.Append(trimmed);
        }

        if (inCodeFence)
        {
            AddCodeBlock(document, code.ToString().TrimEnd('\n'));
        }

        FlushParagraph();

        if (document.Blocks.Count == 0)
        {
            AddParagraph(document, string.Empty);
        }

        return document;
    }

    private FlowDocument CreateEmptyDocument()
    {
        var document = new FlowDocument
        {
            PagePadding = new Thickness(22, 18, 28, 24),
            FontFamily = new FontFamily("Segoe UI"),
            FontSize = 15,
            Foreground = GetBrush("TextBrush"),
            Background = GetBrush("PanelAltBrush"),
            LineHeight = 22
        };

        ApplyDocumentWordWrap(document, WordWrapBox.IsChecked == true);
        return document;
    }

    private void AddHeading(FlowDocument document, int level, string text)
    {
        var sizes = new[] { 30.0, 25.0, 21.0, 18.0, 16.0, 15.0 };
        var paragraph = new Paragraph
        {
            FontSize = sizes[Math.Clamp(level - 1, 0, sizes.Length - 1)],
            FontWeight = FontWeights.SemiBold,
            Foreground = GetBrush("TextBrush"),
            Margin = new Thickness(0, level <= 2 ? 14 : 10, 0, 6)
        };

        AddInlineRuns(paragraph.Inlines, text);
        RegisterHeadingAnchor(text, paragraph);
        document.Blocks.Add(paragraph);
    }

    private void AddParagraph(FlowDocument document, string text)
    {
        var paragraph = new Paragraph
        {
            Margin = new Thickness(0, 0, 0, 8),
            Foreground = GetBrush("TextBrush")
        };

        AddInlineRuns(paragraph.Inlines, text);
        document.Blocks.Add(paragraph);
    }

    private void AddBlockquote(FlowDocument document, string text)
    {
        var paragraph = new Paragraph
        {
            Margin = new Thickness(16, 2, 0, 10),
            Foreground = GetBrush("MutedTextBrush"),
            FontStyle = FontStyles.Italic
        };

        paragraph.Inlines.Add(new Run("> ") { Foreground = GetBrush("AccentBrush"), FontStyle = FontStyles.Normal });
        AddInlineRuns(paragraph.Inlines, text);
        document.Blocks.Add(paragraph);
    }

    private void AddListItem(FlowDocument document, string marker, string text)
    {
        var paragraph = new Paragraph
        {
            Margin = new Thickness(18, 0, 0, 4),
            Foreground = GetBrush("TextBrush")
        };

        paragraph.Inlines.Add(new Run(marker + " ") { Foreground = GetBrush("AccentBrush"), FontWeight = FontWeights.SemiBold });
        AddInlineRuns(paragraph.Inlines, text);
        document.Blocks.Add(paragraph);
    }

    private void AddTable(FlowDocument document, MarkdownTable tableData)
    {
        var columnCount = tableData.ColumnCount;
        if (columnCount == 0)
        {
            return;
        }

        var table = new Table
        {
            CellSpacing = 0,
            Margin = new Thickness(0, 8, 0, 14),
            BorderBrush = GetBrush("BorderBrushSoft"),
            BorderThickness = new Thickness(1)
        };

        for (var i = 0; i < columnCount; i++)
        {
            table.Columns.Add(new TableColumn());
        }

        var headerGroup = new TableRowGroup();
        var headerRow = new TableRow();
        foreach (var header in NormalizeTableRow(tableData.Header, columnCount))
        {
            headerRow.Cells.Add(CreateTableCell(header, true));
        }

        headerGroup.Rows.Add(headerRow);
        table.RowGroups.Add(headerGroup);

        var bodyGroup = new TableRowGroup();
        foreach (var row in tableData.Rows)
        {
            var tableRow = new TableRow();
            foreach (var cell in NormalizeTableRow(row, columnCount))
            {
                tableRow.Cells.Add(CreateTableCell(cell, false));
            }

            bodyGroup.Rows.Add(tableRow);
        }

        table.RowGroups.Add(bodyGroup);
        document.Blocks.Add(table);
    }

    private TableCell CreateTableCell(string text, bool isHeader)
    {
        var paragraph = new Paragraph
        {
            Margin = new Thickness(0),
            Foreground = GetBrush("TextBrush"),
            FontWeight = isHeader ? FontWeights.SemiBold : FontWeights.Normal
        };

        AddInlineRuns(paragraph.Inlines, text);

        return new TableCell(paragraph)
        {
            Background = isHeader ? GetBrush("CodeBrush") : Brushes.Transparent,
            BorderBrush = GetBrush("BorderBrushSoft"),
            BorderThickness = new Thickness(0, 0, 1, 1),
            Padding = new Thickness(8, 6, 8, 6)
        };
    }

    private void AddCodeBlock(FlowDocument document, string text)
    {
        var paragraph = new Paragraph
        {
            Background = GetBrush("CodeBrush"),
            Foreground = GetBrush("TextBrush"),
            FontFamily = new FontFamily("Cascadia Mono,Consolas,Lucida Console"),
            FontSize = 13,
            LineHeight = 19,
            Margin = new Thickness(0, 4, 0, 12),
            Padding = new Thickness(12, 9, 12, 9)
        };

        var codeLines = text.Replace("\r\n", "\n").Replace('\r', '\n').Split('\n');
        for (var i = 0; i < codeLines.Length; i++)
        {
            paragraph.Inlines.Add(new Run(codeLines[i]));
            if (i < codeLines.Length - 1)
            {
                paragraph.Inlines.Add(new LineBreak());
            }
        }

        document.Blocks.Add(paragraph);
    }

    private void AddHorizontalRule(FlowDocument document)
    {
        var paragraph = new Paragraph
        {
            Margin = new Thickness(0, 8, 0, 12)
        };
        paragraph.Inlines.Add(new Run(new string('-', 40)) { Foreground = GetBrush("BorderBrushSoft") });
        document.Blocks.Add(paragraph);
    }

    private void AddInlineRuns(InlineCollection inlines, string text, int depth = 0)
    {
        if (depth > 8)
        {
            inlines.Add(new Run(text));
            return;
        }

        var index = 0;
        while (index < text.Length)
        {
            if (text[index] == '`')
            {
                var end = text.IndexOf('`', index + 1);
                if (end > index)
                {
                    inlines.Add(new Run(text[(index + 1)..end])
                    {
                        Background = GetBrush("CodeBrush"),
                        Foreground = GetBrush("AccentBrush"),
                        FontFamily = new FontFamily("Cascadia Mono,Consolas,Lucida Console"),
                        FontSize = 13
                    });
                    index = end + 1;
                    continue;
                }
            }

            if (index + 1 < text.Length && text[index] == '*' && text[index + 1] == '*')
            {
                var end = text.IndexOf("**", index + 2, StringComparison.Ordinal);
                if (end > index)
                {
                    var bold = new Bold();
                    AddInlineRuns(bold.Inlines, text[(index + 2)..end], depth + 1);
                    inlines.Add(bold);
                    index = end + 2;
                    continue;
                }
            }

            if (text[index] == '*')
            {
                var end = text.IndexOf('*', index + 1);
                if (end > index)
                {
                    var italic = new Italic();
                    AddInlineRuns(italic.Inlines, text[(index + 1)..end], depth + 1);
                    inlines.Add(italic);
                    index = end + 1;
                    continue;
                }
            }

            if ((text[index] == '[' || (text[index] == '!' && index + 1 < text.Length && text[index + 1] == '[')) &&
                TryReadLink(text, index, out var consumed, out var label, out var url))
            {
                var hyperlink = new Hyperlink
                {
                    Foreground = GetBrush("AccentBrush"),
                    TextDecorations = TextDecorations.Underline,
                    ToolTip = url,
                    Tag = url,
                    NavigateUri = CreateNavigateUri(url)
                };
                AddInlineRuns(hyperlink.Inlines, label, depth + 1);
                hyperlink.RequestNavigate += Hyperlink_RequestNavigate;
                inlines.Add(hyperlink);
                index += consumed;
                continue;
            }

            var next = FindNextInlineMarker(text, index + 1);
            var length = next < 0 ? text.Length - index : next - index;
            inlines.Add(new Run(text.Substring(index, length)));
            index += length;
        }
    }

    private void RegisterHeadingAnchor(string headingText, Paragraph paragraph)
    {
        var slug = CreateHeadingSlug(headingText);
        if (string.IsNullOrEmpty(slug))
        {
            return;
        }

        if (_headingAnchorCounts.TryGetValue(slug, out var count))
        {
            count++;
            _headingAnchorCounts[slug] = count;
            slug = $"{slug}-{count}";
        }
        else
        {
            _headingAnchorCounts[slug] = 0;
        }

        _headingAnchors[slug] = paragraph;
    }

    private async void Hyperlink_RequestNavigate(object sender, RequestNavigateEventArgs e)
    {
        e.Handled = true;

        var target = sender is Hyperlink { Tag: string rawTarget }
            ? rawTarget
            : e.Uri.OriginalString;

        if (TryJumpToAnchor(target))
        {
            return;
        }

        if (await TryOpenLocalLinkAsync(target))
        {
            return;
        }

        if (TryOpenExternalLink(target))
        {
            return;
        }

        StatusText.Text = "Could not open link";
    }

    private bool TryJumpToAnchor(string target)
    {
        var hashIndex = target.IndexOf('#');
        if (hashIndex < 0 || hashIndex > 0)
        {
            return false;
        }

        var anchor = SafeUnescapeDataString(target[(hashIndex + 1)..]).Trim();
        if (string.IsNullOrWhiteSpace(anchor))
        {
            return true;
        }

        var candidates = new[]
        {
            anchor,
            CreateHeadingSlug(anchor)
        };

        foreach (var candidate in candidates.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            if (_headingAnchors.TryGetValue(candidate, out var paragraph))
            {
                Viewer.Focus();
                BringTextPointerIntoView(paragraph.ContentStart);
                StatusText.Text = $"Anchor: {candidate}";
                return true;
            }
        }

        StatusText.Text = $"Anchor not found: {anchor}";
        return true;
    }

    private async Task<bool> TryOpenLocalLinkAsync(string target)
    {
        var (pathPart, fragment) = SplitLinkTarget(target);
        if (string.IsNullOrWhiteSpace(pathPart))
        {
            return false;
        }

        if (!TryResolveLocalLinkPath(pathPart, out var fullPath))
        {
            return false;
        }

        if (Directory.Exists(fullPath))
        {
            OpenShellTarget(fullPath, "Opened folder", "Could not open folder");
            return true;
        }

        if (!File.Exists(fullPath))
        {
            StatusText.Text = $"Link target not found: {pathPart}";
            return true;
        }

        if (!IsMarkdownFile(new FileInfo(fullPath)))
        {
            OpenShellTarget(fullPath, "Opened file", "Could not open file");
            return true;
        }

        if (IsCurrentFilePath(fullPath))
        {
            if (!string.IsNullOrWhiteSpace(fragment))
            {
                TryJumpToAnchor("#" + fragment);
            }
            else
            {
                StatusText.Text = "Already on linked file";
            }

            return true;
        }

        RememberCurrentViewState();
        if (!PromptSaveIfDirty())
        {
            return true;
        }

        await LoadFileAsync(fullPath);

        if (!string.IsNullOrWhiteSpace(fragment) && IsCurrentFilePath(fullPath))
        {
            TryJumpToAnchor("#" + fragment);
        }

        return true;
    }

    private static (string PathPart, string Fragment) SplitLinkTarget(string target)
    {
        var trimmed = target.Trim();
        var hashIndex = trimmed.IndexOf('#');
        if (hashIndex < 0)
        {
            return (trimmed, string.Empty);
        }

        return (
            trimmed[..hashIndex],
            SafeUnescapeDataString(trimmed[(hashIndex + 1)..]).Trim());
    }

    private bool TryResolveLocalLinkPath(string pathPart, out string fullPath)
    {
        fullPath = string.Empty;

        try
        {
            var unescaped = SafeUnescapeDataString(pathPart.Trim());

            if (Uri.TryCreate(unescaped, UriKind.Absolute, out var uri))
            {
                if (!uri.IsFile)
                {
                    return false;
                }

                fullPath = Path.GetFullPath(uri.LocalPath);
                return true;
            }

            if (_currentFilePath is null)
            {
                return false;
            }

            var baseDirectory = Path.GetDirectoryName(_currentFilePath);
            if (string.IsNullOrWhiteSpace(baseDirectory))
            {
                return false;
            }

            fullPath = Path.GetFullPath(Path.Combine(baseDirectory, unescaped));
            return true;
        }
        catch (Exception ex) when (ex is ArgumentException or IOException or NotSupportedException or UriFormatException or UnauthorizedAccessException)
        {
            App.LogCrash(ex);
            return false;
        }
    }

    private static bool TryOpenExternalLink(string target)
    {
        var trimmed = target.Trim();
        if (!Uri.TryCreate(trimmed, UriKind.Absolute, out var uri))
        {
            return false;
        }

        if (uri.IsFile)
        {
            return false;
        }

        try
        {
            Process.Start(new ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true });
            return true;
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException or IOException or UnauthorizedAccessException)
        {
            App.LogCrash(ex);
            return false;
        }
    }

    private void OpenShellTarget(string path, string successStatus, string failureStatus)
    {
        try
        {
            Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
            StatusText.Text = successStatus;
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException or IOException or UnauthorizedAccessException)
        {
            App.LogCrash(ex);
            StatusText.Text = failureStatus;
        }
    }

    private static string SafeUnescapeDataString(string value)
    {
        try
        {
            return Uri.UnescapeDataString(value);
        }
        catch (UriFormatException)
        {
            return value;
        }
    }

    private static Uri CreateNavigateUri(string url)
    {
        var trimmed = url.Trim();
        if (Uri.TryCreate(trimmed, UriKind.RelativeOrAbsolute, out var uri))
        {
            return uri;
        }

        return new Uri("#", UriKind.Relative);
    }

    private static string CreateHeadingSlug(string text)
    {
        var builder = new StringBuilder(text.Length);
        var lastWasHyphen = false;

        foreach (var rawCharacter in text.Trim())
        {
            var character = char.ToLowerInvariant(rawCharacter);
            if (char.IsLetterOrDigit(character))
            {
                builder.Append(character);
                lastWasHyphen = false;
                continue;
            }

            if (character == '_')
            {
                builder.Append(character);
                lastWasHyphen = false;
                continue;
            }

            if (char.IsWhiteSpace(character) || character == '-')
            {
                if (!lastWasHyphen && builder.Length > 0)
                {
                    builder.Append('-');
                    lastWasHyphen = true;
                }
            }
        }

        return builder.ToString().Trim('-');
    }

    private void SelectRenderedText(int offset, int length)
    {
        var start = GetTextPointerAtRunOffset(Viewer.Document.ContentStart, offset);
        var end = GetTextPointerAtRunOffset(start, length);

        Viewer.Focus();
        Viewer.Selection.Select(start, end);
        BringTextPointerIntoView(start);
    }

    private void HighlightRenderedText(int offset, int length)
    {
        ClearRenderedSearchHighlight();

        var start = GetTextPointerAtRunOffset(Viewer.Document.ContentStart, offset);
        var end = GetTextPointerAtRunOffset(start, length);
        var range = new TextRange(start, end);

        range.ApplyPropertyValue(TextElement.BackgroundProperty, GetBrush("SearchHighlightBrush"));
        range.ApplyPropertyValue(TextElement.ForegroundProperty, GetBrush("SearchHighlightTextBrush"));
        _renderedSearchHighlightActive = true;

        Viewer.Focus();
        Viewer.Selection.Select(start, end);
        BringTextPointerIntoView(start);
    }

    private void ClearRenderedSearchHighlight()
    {
        if (!_renderedSearchHighlightActive)
        {
            return;
        }

        RenderMarkdown(Editor.Text);
    }

    private static void BringTextPointerIntoView(TextPointer pointer)
    {
        if (pointer.Paragraph is not null)
        {
            pointer.Paragraph.BringIntoView();
            return;
        }

        if (pointer.Parent is FrameworkContentElement contentElement)
        {
            contentElement.BringIntoView();
        }
    }

    private int GetRenderedSelectionOffset()
    {
        return GetRenderedTextOffset(Viewer.Selection.Start);
    }

    private int GetRenderedTextOffset(TextPointer target)
    {
        var current = Viewer.Document.ContentStart;
        var offset = 0;

        while (current is not null && current.CompareTo(target) < 0)
        {
            if (current.GetPointerContext(LogicalDirection.Forward) == TextPointerContext.Text)
            {
                var text = current.GetTextInRun(LogicalDirection.Forward);
                var runEnd = current.GetPositionAtOffset(text.Length) ?? current;
                if (runEnd.CompareTo(target) >= 0)
                {
                    return offset + new TextRange(current, target).Text.Length;
                }

                offset += text.Length;
                current = runEnd;
            }
            else
            {
                current = current.GetNextContextPosition(LogicalDirection.Forward);
            }
        }

        return offset;
    }

    private TextPointer GetTextPointerAtRunOffset(TextPointer start, int targetOffset)
    {
        if (targetOffset <= 0)
        {
            return start;
        }

        var current = start;
        var offset = 0;

        while (current is not null && current.CompareTo(Viewer.Document.ContentEnd) < 0)
        {
            if (current.GetPointerContext(LogicalDirection.Forward) == TextPointerContext.Text)
            {
                var text = current.GetTextInRun(LogicalDirection.Forward);
                if (offset + text.Length >= targetOffset)
                {
                    return current.GetPositionAtOffset(targetOffset - offset) ?? current;
                }

                offset += text.Length;
                current = current.GetPositionAtOffset(text.Length) ?? current.GetNextContextPosition(LogicalDirection.Forward);
            }
            else
            {
                current = current.GetNextContextPosition(LogicalDirection.Forward);
            }
        }

        return Viewer.Document.ContentEnd;
    }

    private string ExtractTextRuns(FlowDocument document)
    {
        var builder = new StringBuilder();
        var current = document.ContentStart;

        while (current is not null && current.CompareTo(document.ContentEnd) < 0)
        {
            if (current.GetPointerContext(LogicalDirection.Forward) == TextPointerContext.Text)
            {
                var text = current.GetTextInRun(LogicalDirection.Forward);
                builder.Append(text);
                current = current.GetPositionAtOffset(text.Length) ?? current.GetNextContextPosition(LogicalDirection.Forward);
            }
            else
            {
                current = current.GetNextContextPosition(LogicalDirection.Forward);
            }
        }

        return builder.ToString();
    }

    private static bool IsCodeFence(string trimmed, out string fence)
    {
        if (trimmed.StartsWith("```", StringComparison.Ordinal))
        {
            fence = "```";
            return true;
        }

        if (trimmed.StartsWith("~~~", StringComparison.Ordinal))
        {
            fence = "~~~";
            return true;
        }

        fence = string.Empty;
        return false;
    }

    private static bool TryGetHeading(string line, out int level, out string text)
    {
        level = 0;
        text = string.Empty;

        while (level < line.Length && level < 6 && line[level] == '#')
        {
            level++;
        }

        if (level == 0 || level >= line.Length || !char.IsWhiteSpace(line[level]))
        {
            return false;
        }

        text = line[level..].Trim().TrimEnd('#').TrimEnd();
        return true;
    }

    private static bool IsHorizontalRule(string trimmed)
    {
        if (trimmed.Length < 3)
        {
            return false;
        }

        var first = trimmed[0];
        return (first == '-' || first == '*' || first == '_') && trimmed.All(c => c == first || char.IsWhiteSpace(c));
    }

    private static bool TryGetBlockquote(string line, out string text)
    {
        var trimmed = line.TrimStart();
        if (trimmed.StartsWith('>'))
        {
            text = trimmed[1..].TrimStart();
            return true;
        }

        text = string.Empty;
        return false;
    }

    private static bool TryGetListItem(string line, out string marker, out string text)
    {
        var trimmed = line.TrimStart();
        marker = string.Empty;
        text = string.Empty;

        if (trimmed.Length > 2 && (trimmed[0] == '-' || trimmed[0] == '*' || trimmed[0] == '+') && char.IsWhiteSpace(trimmed[1]))
        {
            marker = "-";
            text = trimmed[2..].TrimStart();
            return true;
        }

        var index = 0;
        while (index < trimmed.Length && char.IsDigit(trimmed[index]))
        {
            index++;
        }

        if (index > 0 &&
            index + 1 < trimmed.Length &&
            (trimmed[index] == '.' || trimmed[index] == ')') &&
            char.IsWhiteSpace(trimmed[index + 1]))
        {
            marker = trimmed[..(index + 1)];
            text = trimmed[(index + 2)..].TrimStart();
            return true;
        }

        return false;
    }

    private static bool TryReadTable(string[] lines, int startIndex, out MarkdownTable table, out int consumedLines)
    {
        table = new MarkdownTable(Array.Empty<string>(), new List<string[]>());
        consumedLines = 0;

        if (startIndex + 1 >= lines.Length)
        {
            return false;
        }

        var header = ParseTableCells(lines[startIndex]);
        var separator = ParseTableCells(lines[startIndex + 1]);
        if (header.Length == 0 || separator.Length == 0 || !IsTableSeparator(separator))
        {
            return false;
        }

        var rows = new List<string[]>();
        var index = startIndex + 2;
        while (index < lines.Length)
        {
            var line = lines[index];
            if (string.IsNullOrWhiteSpace(line) || !line.Contains('|'))
            {
                break;
            }

            var cells = ParseTableCells(line);
            if (cells.Length == 0 || IsTableSeparator(cells))
            {
                break;
            }

            rows.Add(cells);
            index++;
        }

        table = new MarkdownTable(header, rows);
        consumedLines = index - startIndex;
        return true;
    }

    private static string[] ParseTableCells(string line)
    {
        var trimmed = line.Trim();
        if (!trimmed.Contains('|'))
        {
            return Array.Empty<string>();
        }

        if (trimmed.StartsWith('|'))
        {
            trimmed = trimmed[1..];
        }

        if (trimmed.EndsWith('|'))
        {
            trimmed = trimmed[..^1];
        }

        return trimmed
            .Split('|')
            .Select(cell => cell.Trim())
            .ToArray();
    }

    private static bool IsTableSeparator(string[] cells)
    {
        if (cells.Length == 0)
        {
            return false;
        }

        foreach (var cell in cells)
        {
            var normalized = cell.Replace(" ", string.Empty);
            if (normalized.Length < 3)
            {
                return false;
            }

            var withoutAlignment = normalized.Trim(':');
            if (withoutAlignment.Length < 3 || withoutAlignment.Any(c => c != '-'))
            {
                return false;
            }
        }

        return true;
    }

    private static IEnumerable<string> NormalizeTableRow(string[] cells, int columnCount)
    {
        for (var i = 0; i < columnCount; i++)
        {
            yield return i < cells.Length ? cells[i] : string.Empty;
        }
    }

    private static int FindNextInlineMarker(string text, int start)
    {
        for (var i = start; i < text.Length; i++)
        {
            if (text[i] is '`' or '*' or '[' or '!')
            {
                return i;
            }
        }

        return -1;
    }

    private static bool TryReadLink(string text, int start, out int consumed, out string label, out string url)
    {
        var linkStart = text[start] == '!' ? start + 1 : start;
        consumed = 0;
        label = string.Empty;
        url = string.Empty;

        var labelEnd = text.IndexOf(']', linkStart + 1);
        if (labelEnd < 0 || labelEnd + 1 >= text.Length || text[labelEnd + 1] != '(')
        {
            return false;
        }

        var urlEnd = text.IndexOf(')', labelEnd + 2);
        if (urlEnd < 0)
        {
            return false;
        }

        label = text[(linkStart + 1)..labelEnd];
        url = text[(labelEnd + 2)..urlEnd];
        consumed = urlEnd - start + 1;
        return label.Length > 0;
    }

    private void RevealFileInTree(string filePath)
    {
        if (_rootPath is null ||
            FileTree.Items.Count == 0 ||
            !IsPathInsideRoot(filePath, _rootPath) ||
            FileTree.Items[0] is not TreeViewItem root)
        {
            return;
        }

        var relativePath = Path.GetRelativePath(_rootPath, filePath);
        var parts = relativePath.Split(
            new[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar },
            StringSplitOptions.RemoveEmptyEntries);

        if (parts.Length == 0)
        {
            return;
        }

        var current = root;
        current.IsExpanded = true;
        LoadDirectoryChildren(current);

        for (var i = 0; i < parts.Length - 1; i++)
        {
            var nextDirectory = FindChildDirectory(current, parts[i]);
            if (nextDirectory is null)
            {
                return;
            }

            nextDirectory.IsExpanded = true;
            LoadDirectoryChildren(nextDirectory);
            current = nextDirectory;
        }

        var fileItem = FindChildFile(current, filePath);
        if (fileItem is null)
        {
            return;
        }

        try
        {
            _suppressTreeSelection = true;
            fileItem.IsSelected = true;
            fileItem.BringIntoView();
        }
        finally
        {
            _suppressTreeSelection = false;
        }
    }

    private static TreeViewItem? FindChildDirectory(TreeViewItem parent, string directoryName)
    {
        foreach (var child in parent.Items.OfType<TreeViewItem>())
        {
            if (child.Tag is NodeInfo { IsDirectory: true } node &&
                string.Equals(Path.GetFileName(node.Path), directoryName, StringComparison.OrdinalIgnoreCase))
            {
                return child;
            }
        }

        return null;
    }

    private static TreeViewItem? FindChildFile(TreeViewItem parent, string filePath)
    {
        foreach (var child in parent.Items.OfType<TreeViewItem>())
        {
            if (child.Tag is NodeInfo { IsDirectory: false } node &&
                string.Equals(Path.GetFullPath(node.Path), Path.GetFullPath(filePath), StringComparison.OrdinalIgnoreCase))
            {
                return child;
            }
        }

        return null;
    }

    private NodeInfo? GetTreeContextNode()
    {
        if (_treeContextNode is not null)
        {
            return _treeContextNode;
        }

        return FileTree.SelectedItem is TreeViewItem item ? item.Tag as NodeInfo : null;
    }

    private void ShowInFileExplorer(NodeInfo node)
    {
        try
        {
            if (node.IsDirectory)
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = node.Path,
                    UseShellExecute = true
                });
                StatusText.Text = "Opened folder in File Explorer";
                return;
            }

            Process.Start(new ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = $"/select,\"{node.Path}\"",
                UseShellExecute = true
            });
            StatusText.Text = "Opened file in File Explorer";
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
        {
            MessageBox.Show(this, ex.Message, "MarkRush", MessageBoxButton.OK, MessageBoxImage.Error);
            StatusText.Text = "Could not open File Explorer";
        }
    }

    private void RememberCurrentViewState()
    {
        if (_currentFilePath is null)
        {
            return;
        }

        var key = NormalizeViewStatePath(_currentFilePath);
        TryGetFileViewState(_currentFilePath, out var previousState);

        var viewerScroll = GetDocumentScrollViewer(Viewer);
        var editorScroll = GetDocumentScrollViewer(Editor);
        var viewerTextOffset = previousState?.ViewerTextOffset ?? 0;
        var editorTopLineIndex = previousState?.EditorTopLineIndex ?? 0;
        var editorTopTextOffset = previousState?.EditorTopTextOffset ?? 0;

        if (Viewer.Visibility == Visibility.Visible && Viewer.IsEnabled)
        {
            var pointer = GetRenderedViewportPointer() ?? Viewer.Selection.Start;
            viewerTextOffset = Math.Clamp(GetRenderedTextOffset(pointer), 0, _renderedText.Length);
        }

        if (Editor.Visibility == Visibility.Visible && Editor.IsEnabled)
        {
            var firstVisibleLine = Editor.GetFirstVisibleLineIndex();
            if (firstVisibleLine >= 0 && firstVisibleLine < Editor.LineCount)
            {
                editorTopLineIndex = firstVisibleLine;
                editorTopTextOffset = Editor.GetCharacterIndexFromLineIndex(firstVisibleLine);
            }
            else
            {
                editorTopTextOffset = Math.Clamp(Editor.CaretIndex, 0, Editor.Text.Length);
                editorTopLineIndex = Editor.GetLineIndexFromCharacterIndex(editorTopTextOffset);
            }
        }

        _settings.FileViewStates[key] = new FileViewState
        {
            ViewerHorizontalOffset = viewerScroll?.HorizontalOffset ?? previousState?.ViewerHorizontalOffset ?? 0,
            ViewerVerticalOffset = viewerScroll?.VerticalOffset ?? previousState?.ViewerVerticalOffset ?? 0,
            ViewerTextOffset = viewerTextOffset,
            EditorHorizontalOffset = editorScroll?.HorizontalOffset ?? previousState?.EditorHorizontalOffset ?? 0,
            EditorVerticalOffset = editorScroll?.VerticalOffset ?? previousState?.EditorVerticalOffset ?? 0,
            EditorCaretIndex = Math.Clamp(Editor.CaretIndex, 0, Editor.Text.Length),
            EditorTopLineIndex = Math.Max(0, editorTopLineIndex),
            EditorTopTextOffset = Math.Clamp(editorTopTextOffset, 0, Editor.Text.Length),
            LastTouchedUtc = DateTime.UtcNow
        };

        TrimStoredFileViewStates();
    }

    private void RestoreCurrentViewState()
    {
        var expectedPath = _currentFilePath;
        if (expectedPath is null)
        {
            return;
        }

        if (!TryGetFileViewState(expectedPath, out var viewState) || viewState is null)
        {
            Editor.CaretIndex = 0;
            Dispatcher.BeginInvoke(
                () =>
                {
                    if (!IsCurrentFilePath(expectedPath))
                    {
                        return;
                    }

                    Editor.ScrollToHome();
                    Viewer.ScrollToHome();
                    FocusDocumentSurface();
                },
                DispatcherPriority.ContextIdle);
            return;
        }

        Editor.CaretIndex = Math.Clamp(viewState.EditorCaretIndex, 0, Editor.Text.Length);

        Dispatcher.BeginInvoke(
            () =>
            {
                if (!IsCurrentFilePath(expectedPath))
                {
                    return;
                }

                RestoreEditorViewState(viewState);
                RestoreRenderedViewState(viewState);
                FocusDocumentSurface();
            },
            DispatcherPriority.ContextIdle);
    }

    private void RestoreEditorViewState(FileViewState viewState)
    {
        var targetLine = Math.Clamp(viewState.EditorTopLineIndex, 0, Math.Max(0, Editor.LineCount - 1));
        if (viewState.EditorTopTextOffset > 0 || viewState.EditorTopLineIndex > 0)
        {
            Editor.ScrollToLine(targetLine);
            return;
        }

        ScrollDocumentSurface(Editor, viewState.EditorHorizontalOffset, viewState.EditorVerticalOffset);
    }

    private void RestoreRenderedViewState(FileViewState viewState)
    {
        var targetOffset = Math.Clamp(viewState.ViewerTextOffset, 0, _renderedText.Length);
        if (targetOffset > 0)
        {
            var pointer = GetTextPointerAtRunOffset(Viewer.Document.ContentStart, targetOffset);
            BringTextPointerIntoView(pointer);
            return;
        }

        ScrollDocumentSurface(Viewer, viewState.ViewerHorizontalOffset, viewState.ViewerVerticalOffset);
    }

    private bool TryGetFileViewState(string path, out FileViewState? viewState)
    {
        var key = NormalizeViewStatePath(path);
        if (_settings.FileViewStates.TryGetValue(key, out viewState))
        {
            return true;
        }

        return _settings.FileViewStates.TryGetValue(path, out viewState);
    }

    private static ScrollViewer? GetDocumentScrollViewer(DependencyObject surface)
    {
        if (surface is FrameworkElement element)
        {
            element.ApplyTemplate();
        }

        return FindVisualChild<ScrollViewer>(surface);
    }

    private static void ScrollDocumentSurface(DependencyObject surface, double horizontalOffset, double verticalOffset)
    {
        var scrollViewer = GetDocumentScrollViewer(surface);
        if (scrollViewer is null)
        {
            return;
        }

        scrollViewer.ScrollToHorizontalOffset(NormalizeScrollOffset(horizontalOffset));
        scrollViewer.ScrollToVerticalOffset(NormalizeScrollOffset(verticalOffset));
    }

    private static double GetScrollRatio(ScrollViewer? scrollViewer)
    {
        if (scrollViewer is null || scrollViewer.ScrollableHeight <= 0)
        {
            return 0;
        }

        return Math.Clamp(scrollViewer.VerticalOffset / scrollViewer.ScrollableHeight, 0, 1);
    }

    private static void ScrollDocumentSurfaceToRatio(DependencyObject surface, double scrollRatio)
    {
        var scrollViewer = GetDocumentScrollViewer(surface);
        if (scrollViewer is null)
        {
            return;
        }

        scrollViewer.ScrollToVerticalOffset(scrollViewer.ScrollableHeight * Math.Clamp(scrollRatio, 0, 1));
    }

    private static double NormalizeScrollOffset(double offset)
    {
        return double.IsFinite(offset) ? Math.Max(0, offset) : 0;
    }

    private static string NormalizeViewStatePath(string path)
    {
        try
        {
            return Path.GetFullPath(path);
        }
        catch (Exception ex) when (ex is ArgumentException or IOException or NotSupportedException or UnauthorizedAccessException)
        {
            return path;
        }
    }

    private bool IsCurrentFilePath(string path)
    {
        return _currentFilePath is not null &&
               string.Equals(
                   NormalizeViewStatePath(_currentFilePath),
                   NormalizeViewStatePath(path),
                   StringComparison.OrdinalIgnoreCase);
    }

    private void TrimStoredFileViewStates()
    {
        if (_settings.FileViewStates.Count <= MaxStoredFileViewStates)
        {
            return;
        }

        var keysToRemove = _settings.FileViewStates
            .OrderBy(pair => pair.Value.LastTouchedUtc)
            .Take(_settings.FileViewStates.Count - MaxStoredFileViewStates)
            .Select(pair => pair.Key)
            .ToArray();

        foreach (var key in keysToRemove)
        {
            _settings.FileViewStates.Remove(key);
        }
    }

    private ModeSwitchAnchor? CaptureModeSwitchAnchor(bool editing)
    {
        if (_currentFilePath is null)
        {
            return null;
        }

        return editing ? CaptureRenderedAnchor() : CaptureEditorAnchor();
    }

    private ModeSwitchAnchor CaptureRenderedAnchor()
    {
        var scrollViewer = GetDocumentScrollViewer(Viewer);
        var scrollRatio = GetScrollRatio(scrollViewer);
        var pointer = GetRenderedViewportPointer() ?? Viewer.Selection.Start;
        var offset = GetRenderedTextOffset(pointer);

        if (offset <= 0 && scrollRatio > 0)
        {
            offset = (int)Math.Round(Editor.Text.Length * scrollRatio);
        }

        return new ModeSwitchAnchor(Math.Clamp(offset, 0, Editor.Text.Length), scrollRatio);
    }

    private ModeSwitchAnchor CaptureEditorAnchor()
    {
        var scrollViewer = GetDocumentScrollViewer(Editor);
        var scrollRatio = GetScrollRatio(scrollViewer);
        var offset = Math.Clamp(Editor.CaretIndex, 0, Editor.Text.Length);
        var firstVisibleLine = Editor.GetFirstVisibleLineIndex();

        if (firstVisibleLine >= 0 && firstVisibleLine < Editor.LineCount)
        {
            offset = Editor.GetCharacterIndexFromLineIndex(firstVisibleLine);
        }

        return new ModeSwitchAnchor(offset, scrollRatio);
    }

    private TextPointer? GetRenderedViewportPointer()
    {
        Viewer.UpdateLayout();
        var x = Math.Clamp(Viewer.ActualWidth * 0.08, 24, Math.Max(24, Viewer.ActualWidth - 24));
        var y = Math.Clamp(Viewer.ActualHeight * 0.08, 24, Math.Max(24, Viewer.ActualHeight - 24));
        return Viewer.GetPositionFromPoint(new Point(x, y), true);
    }

    private void RestoreEditorAnchor(ModeSwitchAnchor? anchor)
    {
        if (anchor is null)
        {
            Editor.Focus();
            return;
        }

        var targetOffset = Math.Clamp(anchor.TextOffset, 0, Editor.Text.Length);
        Editor.CaretIndex = targetOffset;

        Dispatcher.BeginInvoke(
            () =>
            {
                if (!IsEditing)
                {
                    return;
                }

                var targetLine = Editor.GetLineIndexFromCharacterIndex(targetOffset);
                if (targetLine >= 0 && targetLine < Editor.LineCount)
                {
                    Editor.ScrollToLine(targetLine);
                }
                else
                {
                    ScrollDocumentSurfaceToRatio(Editor, anchor.ScrollRatio);
                }

                Editor.Focus();
            },
            DispatcherPriority.ContextIdle);
    }

    private void RestoreRenderedAnchor(ModeSwitchAnchor? anchor)
    {
        if (anchor is null)
        {
            Viewer.Focus();
            return;
        }

        var targetOffset = Math.Clamp(anchor.TextOffset, 0, _renderedText.Length);

        Dispatcher.BeginInvoke(
            () =>
            {
                if (IsEditing)
                {
                    return;
                }

                if (targetOffset > 0)
                {
                    var pointer = GetTextPointerAtRunOffset(Viewer.Document.ContentStart, targetOffset);
                    BringTextPointerIntoView(pointer);
                }
                else
                {
                    ScrollDocumentSurfaceToRatio(Viewer, anchor.ScrollRatio);
                }

                Viewer.Focus();
            },
            DispatcherPriority.ContextIdle);
    }

    private void UpdateEditorMode()
    {
        var editing = EditModeBox.IsChecked == true;
        var anchor = CaptureModeSwitchAnchor(editing);
        RememberCurrentViewState();

        Editor.IsReadOnly = !editing;
        Editor.Visibility = editing ? Visibility.Visible : Visibility.Collapsed;
        Viewer.Visibility = editing ? Visibility.Collapsed : Visibility.Visible;

        if (editing)
        {
            RestoreEditorAnchor(anchor);
        }
        else
        {
            RenderMarkdown(Editor.Text);
            RestoreRenderedAnchor(anchor);
        }

        RebuildSearchIndex();
        StatusText.Text = editing ? "Editing" : "Rendered";
        UpdateStats();
    }

    private bool EnsureEditorCanModify()
    {
        if (_currentFilePath is null)
        {
            return false;
        }

        if (EditModeBox.IsChecked != true)
        {
            EditModeBox.IsChecked = true;
        }

        Editor.IsReadOnly = false;
        Editor.Visibility = Visibility.Visible;
        Viewer.Visibility = Visibility.Collapsed;
        Editor.Focus();
        return true;
    }

    private void FocusDocumentSurface()
    {
        if (IsEditing)
        {
            Editor.Focus();
        }
        else
        {
            Viewer.Focus();
        }
    }

    private bool IsDocumentSurfaceFocused()
    {
        return Editor.IsKeyboardFocusWithin || Viewer.IsKeyboardFocusWithin;
    }

    private void ApplyTheme(AppTheme theme)
    {
        SetBrush("PanelBrush", theme.Panel);
        SetBrush("PanelAltBrush", theme.PanelAlt);
        SetBrush("BorderBrushSoft", theme.Border);
        SetBrush("TextBrush", theme.Text);
        SetBrush("MutedTextBrush", theme.Muted);
        SetBrush("InputBrush", theme.Input);
        SetBrush("AccentBrush", theme.Accent);
        SetBrush("ButtonBrush", theme.Button);
        SetBrush("ButtonHoverBrush", theme.ButtonHover);
        SetBrush("ButtonPressedBrush", theme.ButtonPressed);
        SetBrush("ButtonBorderBrush", theme.ButtonBorder);
        SetBrush("SelectionBrush", theme.Selection);
        SetBrush("SearchHighlightBrush", theme.SearchHighlight);
        SetBrush("SearchHighlightTextBrush", theme.SearchHighlightText);
        SetBrush("CodeBrush", theme.Code);

        Background = new SolidColorBrush((Color)ColorConverter.ConvertFromString(theme.Window));
    }

    private void SetBrush(string key, string color)
    {
        var parsed = (Color)ColorConverter.ConvertFromString(color);
        Resources[key] = new SolidColorBrush(parsed);
    }

    private Brush GetBrush(string key)
    {
        return (Brush)FindResource(key);
    }

    private static AppTheme FindTheme(string? name)
    {
        return Themes.FirstOrDefault(theme => string.Equals(theme.Name, name, StringComparison.OrdinalIgnoreCase)) ?? Themes[0];
    }

    private void UpdateCommandState()
    {
        var hasFile = _currentFilePath is not null;
        var hasSearch = hasFile && !string.IsNullOrEmpty(SearchBox.Text);

        ReloadButton.IsEnabled = hasFile;
        SaveButton.IsEnabled = hasFile && (_isDirty || EditModeBox.IsChecked == true);
        CopyFilePathButton.IsEnabled = hasFile;
        SearchBox.IsEnabled = hasFile;
        PreviousMatchButton.IsEnabled = hasSearch;
        NextMatchButton.IsEnabled = hasSearch;
    }

    private void UpdateWindowTitle()
    {
        var marker = _isDirty ? "*" : string.Empty;
        var fileName = _currentFilePath is null ? "MarkRush" : Path.GetFileName(_currentFilePath);
        Title = $"{marker}{fileName} - MarkRush";
    }

    private void UpdateStats()
    {
        var text = Editor.Text;
        var lines = text.Length == 0 ? 0 : text.Count(c => c == '\n') + 1;
        var mode = IsEditing ? "source" : "rendered";
        StatsText.Text = $"{lines:N0} lines, {text.Length:N0} chars, {mode}";
    }

    private void ApplyWindowSettings()
    {
        if (_settings.WindowWidth >= MinWidth)
        {
            Width = _settings.WindowWidth;
        }

        if (_settings.WindowHeight >= MinHeight)
        {
            Height = _settings.WindowHeight;
        }
    }

    private StartupTarget ResolveStartupTarget()
    {
        foreach (var arg in Environment.GetCommandLineArgs().Skip(1))
        {
            if (string.IsNullOrWhiteSpace(arg))
            {
                continue;
            }

            string path;
            try
            {
                path = Path.GetFullPath(arg);
            }
            catch (Exception ex) when (ex is ArgumentException or IOException or NotSupportedException or UnauthorizedAccessException)
            {
                App.LogCrash(ex);
                continue;
            }

            if (File.Exists(path))
            {
                var root = Path.GetDirectoryName(path);
                if (root is not null)
                {
                    return new StartupTarget(root, path);
                }
            }

            if (Directory.Exists(path))
            {
                return new StartupTarget(path, null);
            }
        }

        if (_settings.LastRootPath is not null && Directory.Exists(_settings.LastRootPath))
        {
            var fileInsideSavedRoot = _settings.LastFilePath is not null &&
                                      File.Exists(_settings.LastFilePath) &&
                                      IsPathInsideRoot(_settings.LastFilePath, _settings.LastRootPath);

            return new StartupTarget(
                _settings.LastRootPath,
                fileInsideSavedRoot ? _settings.LastFilePath : null);
        }

        if (_settings.LastFilePath is not null && File.Exists(_settings.LastFilePath))
        {
            var root = Path.GetDirectoryName(_settings.LastFilePath);
            if (root is not null && Directory.Exists(root))
            {
                return new StartupTarget(root, _settings.LastFilePath);
            }
        }

        var documents = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
        if (Directory.Exists(documents))
        {
            return new StartupTarget(documents, null);
        }

        return new StartupTarget(Environment.CurrentDirectory, null);
    }

    private void SaveSettings()
    {
        try
        {
            _settings.LastRootPath = _rootPath;
            _settings.LastFilePath = _currentFilePath;
            _settings.WordWrap = WordWrapBox.IsChecked == true;
            _settings.ThemeName = ThemeBox.SelectedItem as string ?? _settings.ThemeName;
            _settings.WindowWidth = RestoreBounds.Width;
            _settings.WindowHeight = RestoreBounds.Height;

            var directory = Path.GetDirectoryName(SettingsPath);
            if (directory is not null)
            {
                Directory.CreateDirectory(directory);
            }

            File.WriteAllText(SettingsPath, JsonSerializer.Serialize(_settings, JsonOptions));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            StatusText.Text = "Settings were not saved";
        }
    }

    private static AppSettings LoadSettings()
    {
        try
        {
            if (!File.Exists(SettingsPath))
            {
                return new AppSettings();
            }

            var json = File.ReadAllText(SettingsPath);
            var settings = JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            settings.FileViewStates = new Dictionary<string, FileViewState>(
                settings.FileViewStates ?? new Dictionary<string, FileViewState>(),
                StringComparer.OrdinalIgnoreCase);
            return settings;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            return new AppSettings();
        }
    }

    private static async Task<string> ReadFileTextAsync(string path)
    {
        await using var stream = new FileStream(
            path,
            FileMode.Open,
            FileAccess.Read,
            FileShare.ReadWrite | FileShare.Delete,
            StreamBufferSize,
            FileOptions.SequentialScan | FileOptions.Asynchronous);
        using var reader = new StreamReader(stream, Encoding.UTF8, true, StreamBufferSize);
        return await reader.ReadToEndAsync();
    }

    private static bool HasLoadingMarker(TreeViewItem item)
    {
        return item.Items.Count == 1 && item.Items[0] is string marker && marker == LoadingMarker;
    }

    private static bool IsMarkdownFile(FileInfo file)
    {
        return MarkdownExtensions.Contains(file.Extension);
    }

    private static bool IsVisibleDirectory(DirectoryInfo directory)
    {
        return !directory.Attributes.HasFlag(FileAttributes.Hidden) &&
               !directory.Attributes.HasFlag(FileAttributes.System);
    }

    private static bool IsPathInsideRoot(string path, string rootPath)
    {
        var fullPath = Path.GetFullPath(path);
        var fullRoot = Path.GetFullPath(rootPath);

        if (!fullRoot.EndsWith(Path.DirectorySeparatorChar))
        {
            fullRoot += Path.DirectorySeparatorChar;
        }

        return fullPath.StartsWith(fullRoot, StringComparison.OrdinalIgnoreCase);
    }

    private static T? FindVisualChild<T>(DependencyObject parent) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T typedChild)
            {
                return typedChild;
            }

            var nestedChild = FindVisualChild<T>(child);
            if (nestedChild is not null)
            {
                return nestedChild;
            }
        }

        return null;
    }

    private static T? FindVisualParent<T>(DependencyObject source) where T : DependencyObject
    {
        DependencyObject? current = source;
        while (current is not null)
        {
            if (current is T typedParent)
            {
                return typedParent;
            }

            current = VisualTreeHelper.GetParent(current);
        }

        return null;
    }

    private static string SettingsPath =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "MarkRush",
            "settings.json");

    private sealed record NodeInfo(string Path, bool IsDirectory);

    private sealed record StartupTarget(string RootPath, string? FilePath);

    private sealed record ModeSwitchAnchor(int TextOffset, double ScrollRatio);

    private sealed record MarkdownTable(string[] Header, List<string[]> Rows)
    {
        public int ColumnCount => Math.Max(Header.Length, Rows.Count == 0 ? 0 : Rows.Max(row => row.Length));
    }

    private sealed record AppTheme(
        string Name,
        string Window,
        string Panel,
        string PanelAlt,
        string Border,
        string Text,
        string Muted,
        string Input,
        string Accent,
        string Button,
        string ButtonHover,
        string ButtonPressed,
        string ButtonBorder,
        string Selection,
        string SearchHighlight,
        string SearchHighlightText,
        string Code);

    private sealed class AppSettings
    {
        public string? LastRootPath { get; set; }
        public string? LastFilePath { get; set; }
        public string ThemeName { get; set; } = "EVA Dark";
        public bool WordWrap { get; set; } = true;
        public double WindowWidth { get; set; }
        public double WindowHeight { get; set; }
        public Dictionary<string, FileViewState> FileViewStates { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    }

    private sealed class FileViewState
    {
        public double ViewerHorizontalOffset { get; set; }
        public double ViewerVerticalOffset { get; set; }
        public int ViewerTextOffset { get; set; }
        public double EditorHorizontalOffset { get; set; }
        public double EditorVerticalOffset { get; set; }
        public int EditorCaretIndex { get; set; }
        public int EditorTopLineIndex { get; set; }
        public int EditorTopTextOffset { get; set; }
        public DateTime LastTouchedUtc { get; set; } = DateTime.UtcNow;
    }
}
