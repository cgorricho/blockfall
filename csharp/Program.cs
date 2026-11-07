// BLOCKFALL — C# edition (System.Console + ANSI). See ../SPEC.md.
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

const int W = 10, H = 20, BoardX = 2, BoardY = 2, PanelX = 27;
const string LangName = "[C#]";

var game = new Game();
var screen = new Screen(60, 28);
Term.Enter();
try
{
    var sw = Stopwatch.StartNew();
    while (true)
    {
        bool dirty = false;
        while (Console.KeyAvailable)
        {
            var k = Console.ReadKey(true);
            if (game.HandleKey(k)) return;
            dirty = true;
        }
        if (!game.Over && !game.Paused && game.SinceFall.ElapsedMilliseconds >= game.IntervalMs)
        {
            game.Gravity();
            dirty = true;
        }
        if (dirty || sw.ElapsedMilliseconds > 250) { Render(game, screen); sw.Restart(); }
        Thread.Sleep(16);
    }
}
finally
{
    Term.Leave();
}

static void Render(Game g, Screen s)
{
    s.Clear();
    var gray = new Style(245);
    var white = new Style(15, Bold: true);
    var plain = new Style(-1);

    s.Put(2, 0, " B L O C K F A L L ", white);
    s.Put(22, 0, LangName, gray);

    s.Put(BoardX, BoardY, "┌", gray);
    s.Put(BoardX + 1 + W * 2, BoardY, "┐", gray);
    s.Put(BoardX, BoardY + H + 1, "└", gray);
    s.Put(BoardX + 1 + W * 2, BoardY + H + 1, "┘", gray);
    for (int i = 0; i < W * 2; i++)
    {
        s.Put(BoardX + 1 + i, BoardY, "─", gray);
        s.Put(BoardX + 1 + i, BoardY + H + 1, "─", gray);
    }
    for (int y = 0; y < H; y++)
    {
        s.Put(BoardX, BoardY + 1 + y, "│", gray);
        s.Put(BoardX + 1 + W * 2, BoardY + 1 + y, "│", gray);
    }

    void Cell(int x, int y, string txt, int color) => s.Put(BoardX + 1 + x * 2, BoardY + 1 + y, txt, new Style(color));

    for (int y = 0; y < H; y++)
        for (int x = 0; x < W; x++)
        {
            int k = g.Board[y, x];
            if (k != 0) Cell(x, y, "██", Game.Pieces[k - 1].Color);
            else Cell(x, y, " ·", 240);
        }

    if (!g.Over)
    {
        int col = Game.Pieces[g.Cur.Kind].Color;
        int gy = g.GhostY();
        foreach (var c in g.Cur.Cells) Cell(g.Cur.X + c.X, gy + c.Y, "░░", col);
        foreach (var c in g.Cur.Cells) Cell(g.Cur.X + c.X, g.Cur.Y + c.Y, "██", col);
    }

    s.Put(PanelX, 3, "NEXT", white);
    var nd = Game.Pieces[g.Next];
    foreach (var c in nd.Cells) s.Put(PanelX + c.X * 2, 4 + c.Y, "██", new Style(nd.Color));
    s.Put(PanelX, 9, "SCORE", white);
    s.Put(PanelX, 10, g.Score.ToString(), plain);
    s.Put(PanelX, 12, "LINES", white);
    s.Put(PanelX, 13, g.Lines.ToString(), plain);
    s.Put(PanelX, 15, "LEVEL", white);
    s.Put(PanelX, 16, g.Level.ToString(), plain);
    string[] help = { "←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "Q   quit" };
    for (int i = 0; i < help.Length; i++) s.Put(PanelX, 18 + i, help[i], gray);

    void Center(int row, string txt) => s.Put(BoardX + 1 + (W * 2 - txt.Length) / 2, row, txt, new Style(-1, Bold: true, Reverse: true));
    if (g.Over) { Center(12, " GAME OVER "); Center(13, " R = restart "); }
    else if (g.Paused) Center(12, " PAUSED ");

    s.Flush();
}

record struct Pt(int X, int Y);
record PieceDef(int N, Pt[] Cells, int Color);
record struct Style(int Fg, bool Bold = false, bool Reverse = false);

class Piece
{
    public int Kind;
    public Pt[] Cells = Array.Empty<Pt>();
    public int X, Y;
}

class Game
{
    public const int W = 10, H = 20;

    public static readonly PieceDef[] Pieces =
    {
        new(4, new Pt[] { new(0, 1), new(1, 1), new(2, 1), new(3, 1) }, 14), // I
        new(2, new Pt[] { new(0, 0), new(1, 0), new(0, 1), new(1, 1) }, 11), // O
        new(3, new Pt[] { new(1, 0), new(0, 1), new(1, 1), new(2, 1) }, 13), // T
        new(3, new Pt[] { new(1, 0), new(2, 0), new(0, 1), new(1, 1) }, 10), // S
        new(3, new Pt[] { new(0, 0), new(1, 0), new(1, 1), new(2, 1) }, 9),  // Z
        new(3, new Pt[] { new(0, 0), new(0, 1), new(1, 1), new(2, 1) }, 12), // J
        new(3, new Pt[] { new(2, 0), new(0, 1), new(1, 1), new(2, 1) }, 3),  // L
    };

    public int[,] Board = new int[H, W];
    public Piece Cur = new();
    public int Next;
    public int Score, Lines;
    public bool Paused, Over;
    public Stopwatch SinceFall = new();
    readonly List<int> bag = new();
    readonly Random rng = new();

    public int Level => Lines / 10 + 1;
    public int IntervalMs => Math.Max(80, 800 - (Level - 1) * 70);

    public Game() => Reset();

    int Draw7()
    {
        if (bag.Count == 0)
        {
            var p = Enumerable.Range(0, 7).ToArray();
            rng.Shuffle(p);
            bag.AddRange(p);
        }
        int k = bag[0];
        bag.RemoveAt(0);
        return k;
    }

    public void Reset()
    {
        Board = new int[H, W];
        bag.Clear();
        Score = Lines = 0;
        Paused = Over = false;
        Next = Draw7();
        Spawn();
    }

    void Spawn()
    {
        var d = Pieces[Next];
        Cur = new Piece { Kind = Next, Cells = (Pt[])d.Cells.Clone(), X = (W - d.N) / 2, Y = 0 };
        Next = Draw7();
        SinceFall.Restart();
        if (!Fits(Cur.Cells, Cur.X, Cur.Y)) Over = true;
    }

    bool Fits(Pt[] cells, int x, int y)
    {
        foreach (var c in cells)
        {
            int bx = x + c.X, by = y + c.Y;
            if (bx < 0 || bx >= W || by < 0 || by >= H || Board[by, bx] != 0) return false;
        }
        return true;
    }

    static Pt[] Rotate(Pt[] cells, int n, bool cw) =>
        cells.Select(c => cw ? new Pt(n - 1 - c.Y, c.X) : new Pt(c.Y, n - 1 - c.X)).ToArray();

    bool TryMove(int dx, int dy)
    {
        if (!Fits(Cur.Cells, Cur.X + dx, Cur.Y + dy)) return false;
        Cur.X += dx; Cur.Y += dy;
        return true;
    }

    void TryRotate(bool cw)
    {
        var r = Rotate(Cur.Cells, Pieces[Cur.Kind].N, cw);
        foreach (int k in new[] { 0, -1, 1, -2, 2 })
            if (Fits(r, Cur.X + k, Cur.Y)) { Cur.Cells = r; Cur.X += k; return; }
    }

    void Lock()
    {
        foreach (var c in Cur.Cells) Board[Cur.Y + c.Y, Cur.X + c.X] = Cur.Kind + 1;
        int cleared = 0;
        for (int y = H - 1; y >= 0; y--)
        {
            bool full = true;
            for (int x = 0; x < W; x++) if (Board[y, x] == 0) { full = false; break; }
            if (!full) continue;
            cleared++;
            for (int yy = y; yy > 0; yy--)
                for (int x = 0; x < W; x++) Board[yy, x] = Board[yy - 1, x];
            for (int x = 0; x < W; x++) Board[0, x] = 0;
            y++; // re-check same row
        }
        int[] pts = { 0, 100, 300, 500, 800 };
        Score += pts[cleared] * Level;
        Lines += cleared;
        Spawn();
    }

    public void Gravity()
    {
        if (!TryMove(0, 1)) Lock();
        SinceFall.Restart();
    }

    void HardDrop()
    {
        while (TryMove(0, 1)) Score += 2;
        Lock();
    }

    public int GhostY()
    {
        int y = Cur.Y;
        while (Fits(Cur.Cells, Cur.X, y + 1)) y++;
        return y;
    }

    /// <returns>true when the game should quit.</returns>
    public bool HandleKey(ConsoleKeyInfo k)
    {
        char r = char.ToLowerInvariant(k.KeyChar);
        if (k.Key == ConsoleKey.Escape || r == 'q' || (k.Key == ConsoleKey.C && k.Modifiers.HasFlag(ConsoleModifiers.Control))) return true;
        if (r == 'r') { Reset(); return false; }
        if (r == 'p') { if (!Over) Paused = !Paused; return false; }
        if (Over || Paused) return false;

        if (k.Key == ConsoleKey.LeftArrow || r == 'a') TryMove(-1, 0);
        else if (k.Key == ConsoleKey.RightArrow || r == 'd') TryMove(1, 0);
        else if (k.Key == ConsoleKey.DownArrow || r == 's') { if (TryMove(0, 1)) { Score++; SinceFall.Restart(); } }
        else if (k.Key == ConsoleKey.UpArrow || r == 'w' || r == 'x') TryRotate(true);
        else if (r == 'z') TryRotate(false);
        else if (r == ' ') HardDrop();
        return false;
    }
}

/// Off-screen cell buffer flushed as one ANSI string per frame.
class Screen
{
    readonly int w, h;
    readonly string[,] ch;
    readonly Style[,] st;

    public Screen(int w, int h) { this.w = w; this.h = h; ch = new string[h, w]; st = new Style[h, w]; Clear(); }

    public void Clear()
    {
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++) { ch[y, x] = " "; st[y, x] = new Style(-1); }
    }

    public void Put(int x, int y, string s, Style style)
    {
        foreach (var rune in s.EnumerateRunes())
        {
            if (x >= 0 && x < w && y >= 0 && y < h) { ch[y, x] = rune.ToString(); st[y, x] = style; }
            x++;
        }
    }

    public void Flush()
    {
        var sb = new StringBuilder("\x1b[H");
        Style? cur = null;
        for (int y = 0; y < h; y++)
        {
            sb.Append($"\x1b[{y + 1};1H");
            for (int x = 0; x < w; x++)
            {
                var s = st[y, x];
                if (cur != s)
                {
                    sb.Append("\x1b[0m");
                    if (s.Fg >= 0) sb.Append($"\x1b[38;5;{s.Fg}m");
                    if (s.Bold) sb.Append("\x1b[1m");
                    if (s.Reverse) sb.Append("\x1b[7m");
                    cur = s;
                }
                sb.Append(ch[y, x]);
            }
        }
        sb.Append("\x1b[0m");
        Console.Out.Write(sb.ToString());
        Console.Out.Flush();
    }
}

static class Term
{
    const int STD_OUTPUT_HANDLE = -11;
    const uint ENABLE_VIRTUAL_TERMINAL_PROCESSING = 0x0004;

    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr GetStdHandle(int nStdHandle);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetConsoleMode(IntPtr h, out uint mode);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetConsoleMode(IntPtr h, uint mode);

    public static void Enter()
    {
        Console.OutputEncoding = Encoding.UTF8;
        Console.TreatControlCAsInput = true;
        if (OperatingSystem.IsWindows())
        {
            var hOut = GetStdHandle(STD_OUTPUT_HANDLE);
            if (GetConsoleMode(hOut, out uint m)) SetConsoleMode(hOut, m | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
        Console.Out.Write("\x1b[?1049h\x1b[?25l\x1b[2J");
    }

    public static void Leave()
    {
        Console.Out.Write("\x1b[0m\x1b[?25h\x1b[?1049l");
        Console.Out.Flush();
    }
}
