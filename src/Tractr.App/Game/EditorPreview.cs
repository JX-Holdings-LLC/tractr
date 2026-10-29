using FarmEngine.Interop;
using FarmEngine.Schemas;

namespace Tractr.App.Game;

/// <summary>
/// The editor's shared native preview. The map, palette and content/art thumbnails reuse
/// compiled content and decoded images. Each caller supplies its immutable project before
/// drawing, so an older draft cannot change what another surface renders.
/// </summary>
public sealed class EditorPreview : IDisposable
{
    private RustPreview? _preview;
    private bool _disposed;

    internal RustPreview For(GameProject project)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        if (_preview is null || _preview.IsPoisoned)
        {
            _preview?.Dispose();
            _preview = RustPreview.Create(project);
        }
        else
        {
            _preview.SetProject(project);
        }

        return _preview;
    }

    public void Dispose()
    {
        _disposed = true;
        _preview?.Dispose();
        _preview = null;
    }
}
