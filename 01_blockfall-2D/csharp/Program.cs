// BLOCKFALL — C# edition (System.Console + ANSI). See ../../SPEC.md.
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

const int W = 10, H = 20, BoardX = 2, BoardY = 2, PanelX = 27;
const string LangName = "[C#]";

// ---------- command line ----------
bool aiFlag = false, turboFlag = false;
ulong seedArg = 0;
long bench = 0;
for (int i = 0; i < args.Length; i++)
{
    switch (args[i].TrimStart('-'))
    {
        case "ai": aiFlag = true; break;
        case "turbo": turboFlag = true; break;
        case "seed" when i + 1 < args.Length: seedArg = ulong.Parse(args[++i]); break;
        case "bench" when i + 1 < args.Length: bench = long.Parse(args[++i]); break;
    }
}
uint seed = seedArg != 0 ? (uint)seedArg : (uint)DateTime.UtcNow.Ticks;

if (bench > 0)
{
    var bg = new Game(seed, ai: true);
    var sw0 = Stopwatch.StartNew();
    while (bg.Placed < bench)
    {
        if (bg.Over) bg.Reset();
        bg.AiPlaceNow();
    }
    Console.WriteLine($"lang=cs seed={bg.Seed} pieces={bg.Placed} lines={bg.TotalLines} score={bg.Score} games={bg.Games} ms={sw0.ElapsedMilliseconds}");
    return;
}

var game = new Game(seed, aiFlag) { Turbo = aiFlag && turboFlag };
var screen = new Screen(44, 28);
Term.Enter();
try
{
    Render(game, screen);
    game.WriteStats();
    var statsClock = Stopwatch.StartNew();
    while (true)
    {
        bool dirty = false;
        while (Console.KeyAvailable)
        {
            var k = Console.ReadKey(true);
            if (game.HandleKey(k)) return;
            dirty = true;
        }
        if (game.Ai) dirty |= game.AiUpdate();
        else if (!game.Over && !game.Paused && game.SinceFall.ElapsedMilliseconds >= game.IntervalMs)
        {
            game.Gravity();
            dirty = true;
        }
        if (dirty) Render(game, screen);
        if (statsClock.ElapsedMilliseconds >= 500) { game.WriteStats(); statsClock.Restart(); }
        if (!(game.Ai && game.Turbo && !game.Paused)) Thread.Sleep(16);
    }
}
finally
{
    Term.Leave();
    game.DeleteStats();
}

static void Render(Game g, Screen s)
{
    g.Frames++;
    var th = g.Retro ? Theme.Retro : Theme.Colour;
    int bg = g.Retro ? 0 : -1;
    Style Fg(int c, bool bold = false) => new Style(c, bold, false, bg);
    int BlockCol(int kind) => th.Mono ? th.BlockMono : Game.Pieces[kind].Color;

    var plain = new Style(g.Retro ? 10 : -1, false, false, bg);
    s.Clear(plain);
    var gray = Fg(th.Dim);
    var white = Fg(th.Text, bold: true);

    s.Put(2, 0, " B L O C K F A L L ", white);
    s.Put(22, 0, LangName, gray);
    if (g.Ai) s.Put(30, 0, g.Turbo ? "AI TURBO" : "AI WATCH", gray);

    s.Put(BoardX, BoardY, th.TL, gray);
    s.Put(BoardX + 1 + W * 2, BoardY, th.TR, gray);
    s.Put(BoardX, BoardY + H + 1, th.BL, gray);
    s.Put(BoardX + 1 + W * 2, BoardY + H + 1, th.BR, gray);
    for (int i = 0; i < W * 2; i++)
    {
        s.Put(BoardX + 1 + i, BoardY, th.Horiz, gray);
        s.Put(BoardX + 1 + i, BoardY + H + 1, th.Horiz, gray);
    }
    for (int y = 0; y < H; y++)
    {
        s.Put(BoardX, BoardY + 1 + y, th.Vert, gray);
        s.Put(BoardX + 1 + W * 2, BoardY + 1 + y, th.Vert, gray);
    }

    void Cell(int x, int y, string txt, int color) => s.Put(BoardX + 1 + x * 2, BoardY + 1 + y, txt, Fg(color));

    for (int y = 0; y < H; y++)
        for (int x = 0; x < W; x++)
        {
            int k = g.Board[y, x];
            if (k != 0) Cell(x, y, th.Block, BlockCol(k - 1));
            else Cell(x, y, th.Empty, th.EmptyCol);
        }

    if (!g.Over)
    {
        int col = BlockCol(g.Cur.Kind);
        int ghostCol = th.Mono ? th.GhostMono : col;
        int gy = g.GhostY();
        foreach (var c in g.Cur.Cells) Cell(g.Cur.X + c.X, gy + c.Y, th.Ghost, ghostCol);
        foreach (var c in g.Cur.Cells) Cell(g.Cur.X + c.X, g.Cur.Y + c.Y, th.Block, col);
    }

    s.Put(PanelX, 3, "NEXT", white);
    foreach (var c in Game.Pieces[g.Next].Cells) s.Put(PanelX + c.X * 2, 4 + c.Y, th.Block, Fg(BlockCol(g.Next)));
    s.Put(PanelX, 9, "SCORE", white);
    s.Put(PanelX, 10, g.Score.ToString(), plain);
    s.Put(PanelX, 12, "LINES", white);
    s.Put(PanelX, 13, g.Lines.ToString(), plain);
    s.Put(PanelX, 15, "LEVEL", white);
    s.Put(PanelX, 16, g.Level.ToString(), plain);
    string[] help = { "←→  move", "↑ X rotate", "Z   rotate ccw", "↓   soft drop", "SPC hard drop", "P   pause", "R   restart", "F   retro mode", "Q   quit" };
    for (int i = 0; i < help.Length; i++) s.Put(PanelX, 18 + i, help[i], gray);
    if (g.Ai) s.Put(PanelX, 27, "T   ai turbo", gray);

    void Center(int row, string txt) => s.Put(BoardX + 1 + (W * 2 - txt.Length) / 2, row, txt, plain with { Bold = true, Reverse = true });
    if (g.Over) { Center(12, " GAME OVER "); Center(13, " R = restart "); }
    else if (g.Paused) Center(12, " PAUSED ");

    s.Flush();
}

record struct Pt(int X, int Y);
record PieceDef(int N, Pt[] Cells, int Color);
record struct Style(int Fg, bool Bold = false, bool Reverse = false, int Bg = -1);

/// Everything that changes between the colour and retro looks.
record Theme(string Block, string Ghost, string Empty, string TL, string TR, string BL, string BR, string Horiz, string Vert,
             int Dim, int Text, int EmptyCol, bool Mono, int BlockMono, int GhostMono)
{
    public static readonly Theme Colour = new("██", "░░", " ·", "┌", "┐", "└", "┘", "─", "│", 245, 15, 240, false, 0, 0);
    public static readonly Theme Retro = new("[]", "::", " .", "+", "+", "+", "+", "-", "|", 28, 10, 28, true, 10, 28);
}

class Piece
{
    public int Kind;
    public Pt[] Cells = Array.Empty<Pt>();
    public int X, Y;
}

record struct AiPlan(int Rots, int X, bool Ok);

class Game
{
    public const int W = 10, H = 20;
    const int AiStepMs = 50, TurboBudgetMs = 15;
    static readonly int[] Kicks = { 0, -1, 1, -2, 2 };

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
    public long Score, Lines;
    public bool Paused, Over;
    public bool Retro; // F: monochrome green "[]" rendering
    public Stopwatch SinceFall = new();
    readonly List<int> bag = new();
    uint rng;
    public readonly uint Seed;

    public readonly bool Ai;
    public bool Turbo;
    AiPlan plan;
    int rotsDone;
    readonly Stopwatch sinceAi = new();
    readonly Stopwatch sinceOver = new();

    // running totals, never reset (Lines/Score are per game)
    public long Placed, Games, Frames, TotalLines;

    public long Level => Lines / 10 + 1;
    public int IntervalMs => (int)Math.Max(80, 800 - (Level - 1) * 70);

    public Game(uint seed, bool ai)
    {
        if (seed == 0) seed = 0x9E3779B9;
        rng = Seed = seed;
        Ai = ai;
        Reset();
    }

    // ---------- shared PRNG + 7-bag ----------

    uint NextRand()
    {
        uint x = rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        rng = x;
        return x;
    }

    int Draw7()
    {
        if (bag.Count == 0)
        {
            int[] a = { 0, 1, 2, 3, 4, 5, 6 };
            for (int i = 6; i >= 1; i--)
            {
                int j = (int)(NextRand() % (uint)(i + 1));
                (a[i], a[j]) = (a[j], a[i]);
            }
            bag.AddRange(a);
        }
        int k = bag[0];
        bag.RemoveAt(0);
        return k;
    }

    // ---------- core rules ----------

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
        if (!Fits(Board, Cur.Cells, Cur.X, Cur.Y))
        {
            Over = true;
            Games++;
            sinceOver.Restart();
            return;
        }
        if (Ai)
        {
            plan = FindPlan();
            rotsDone = 0;
            sinceAi.Restart();
        }
    }

    static bool Fits(int[,] b, Pt[] cells, int x, int y)
    {
        foreach (var c in cells)
        {
            int bx = x + c.X, by = y + c.Y;
            if (bx < 0 || bx >= W || by < 0 || by >= H || b[by, bx] != 0) return false;
        }
        return true;
    }

    static Pt[] Rotate(Pt[] cells, int n, bool cw)
    {
        var r = new Pt[cells.Length];
        for (int i = 0; i < cells.Length; i++)
            r[i] = cw ? new Pt(n - 1 - cells[i].Y, cells[i].X) : new Pt(cells[i].Y, n - 1 - cells[i].X);
        return r;
    }

    /// Rotates and applies the wall-kick rule; returns false if nothing fits.
    static bool KickRotate(int[,] b, ref Pt[] cells, int n, ref int x, int y, bool cw)
    {
        var r = Rotate(cells, n, cw);
        foreach (int k in Kicks)
            if (Fits(b, r, x + k, y)) { cells = r; x += k; return true; }
        return false;
    }

    bool TryMove(int dx, int dy)
    {
        if (!Fits(Board, Cur.Cells, Cur.X + dx, Cur.Y + dy)) return false;
        Cur.X += dx; Cur.Y += dy;
        return true;
    }

    void TryRotate(bool cw) => KickRotate(Board, ref Cur.Cells, Pieces[Cur.Kind].N, ref Cur.X, Cur.Y, cw);

    /// Removes full rows and returns how many were cleared.
    static int ClearLines(int[,] b)
    {
        int cleared = 0;
        for (int y = H - 1; y >= 0; y--)
        {
            bool full = true;
            for (int x = 0; x < W; x++) if (b[y, x] == 0) { full = false; break; }
            if (!full) continue;
            cleared++;
            for (int yy = y; yy > 0; yy--)
                for (int x = 0; x < W; x++) b[yy, x] = b[yy - 1, x];
            for (int x = 0; x < W; x++) b[0, x] = 0;
            y++; // re-check same row
        }
        return cleared;
    }

    void Lock()
    {
        foreach (var c in Cur.Cells) Board[Cur.Y + c.Y, Cur.X + c.X] = Cur.Kind + 1;
        int cleared = ClearLines(Board);
        long[] pts = { 0, 100, 300, 500, 800 };
        Score += pts[cleared] * Level;
        Lines += cleared;
        TotalLines += cleared;
        Placed++;
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
        while (Fits(Board, Cur.Cells, Cur.X, y + 1)) y++;
        return y;
    }

    // ---------- AI ----------

    static long Evaluate(int[,] b, int cleared)
    {
        long agg = 0, holes = 0, bump = 0;
        var hts = new long[W];
        for (int x = 0; x < W; x++)
        {
            int top = H;
            for (int y = 0; y < H; y++) if (b[y, x] != 0) { top = y; break; }
            hts[x] = H - top;
            agg += hts[x];
            for (int y = top + 1; y < H; y++) if (b[y, x] == 0) holes++;
        }
        for (int x = 0; x < W - 1; x++) bump += Math.Abs(hts[x] - hts[x + 1]);
        return -510066 * agg + 760666L * cleared - 356630 * holes - 184483 * bump;
    }

    AiPlan FindPlan()
    {
        var best = new AiPlan(0, 0, false);
        long bestScore = 0;
        int n = Pieces[Cur.Kind].N;
        for (int r = 0; r < 4; r++)
        {
            var cells = Cur.Cells;
            int x = Cur.X, y = Cur.Y;
            bool ok = true;
            for (int i = 0; i < r && ok; i++) ok = KickRotate(Board, ref cells, n, ref x, y, true);
            if (!ok) continue;
            for (int tx = -3; tx < W; tx++)
            {
                int px = x;
                bool valid = true;
                while (px != tx)
                {
                    int step = tx < px ? -1 : 1;
                    if (!Fits(Board, cells, px + step, y)) { valid = false; break; }
                    px += step;
                }
                if (!valid) continue;
                int py = y;
                while (Fits(Board, cells, px, py + 1)) py++;
                var b = (int[,])Board.Clone();
                foreach (var c in cells) b[py + c.Y, px + c.X] = 1;
                long s = Evaluate(b, ClearLines(b));
                if (!best.Ok || s > bestScore) { best = new AiPlan(r, tx, true); bestScore = s; }
            }
        }
        return best;
    }

    /// Performs one step of the current plan.
    void AiAction()
    {
        if (!plan.Ok) HardDrop();
        else if (rotsDone < plan.Rots) { TryRotate(true); rotsDone++; }
        else if (Cur.X < plan.X) { if (!TryMove(1, 0)) HardDrop(); }
        else if (Cur.X > plan.X) { if (!TryMove(-1, 0)) HardDrop(); }
        else HardDrop();
    }

    /// Executes the whole plan for the current piece at once.
    public void AiPlaceNow()
    {
        long start = Placed;
        while (Placed == start && !Over) AiAction();
    }

    /// Advances the AI; returns true if anything changed.
    public bool AiUpdate()
    {
        if (Paused) return false;
        if (Over)
        {
            if (Turbo || sinceOver.ElapsedMilliseconds >= 1000) { Reset(); return true; }
            return false;
        }
        if (Turbo)
        {
            var sw = Stopwatch.StartNew();
            while (sw.ElapsedMilliseconds < TurboBudgetMs && !Over) AiPlaceNow();
            return true;
        }
        if (sinceAi.ElapsedMilliseconds >= AiStepMs) { AiAction(); sinceAi.Restart(); return true; }
        return false;
    }

    // ---------- stats file ----------

    static string StatsPath => Path.Combine(Path.GetTempPath(), "blockfall", "cs.json");

    public void WriteStats()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(StatsPath)!);
            string b(bool v) => v ? "true" : "false";
            string json = $"{{\"lang\":\"C#\",\"pid\":{Environment.ProcessId},\"ai\":{b(Ai)},\"turbo\":{b(Turbo)},\"seed\":{Seed}," +
                          $"\"pieces\":{Placed},\"lines\":{TotalLines},\"score\":{Score},\"games\":{Games},\"frames\":{Frames}," +
                          $"\"ts\":{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}}}";
            File.WriteAllText(StatsPath + ".tmp", json);
            File.Move(StatsPath + ".tmp", StatsPath, overwrite: true);
        }
        catch (IOException) { /* dashboard is optional */ }
    }

    public void DeleteStats()
    {
        try { File.Delete(StatsPath); } catch (IOException) { }
    }

    /// <returns>true when the game should quit.</returns>
    public bool HandleKey(ConsoleKeyInfo k)
    {
        char r = char.ToLowerInvariant(k.KeyChar);
        if (k.Key == ConsoleKey.Escape || r == 'q' || (k.Key == ConsoleKey.C && k.Modifiers.HasFlag(ConsoleModifiers.Control))) return true;
        if (r == 'r') { Reset(); return false; }
        if (r == 'f') { Retro = !Retro; return false; }
        if (r == 'p') { if (!Over) Paused = !Paused; return false; }
        if (r == 't') { if (Ai) Turbo = !Turbo; return false; }
        if (Over || Paused || Ai) return false;

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
    Style fill = new(-1);
    int? lastBg; // background the whole terminal was last cleared to

    public Screen(int w, int h) { this.w = w; this.h = h; ch = new string[h, w]; st = new Style[h, w]; Clear(new Style(-1)); }

    public void Clear(Style fillStyle)
    {
        fill = fillStyle;
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++) { ch[y, x] = " "; st[y, x] = fillStyle; }
    }

    static void AppendSgr(StringBuilder sb, Style s)
    {
        sb.Append("\x1b[0m");
        if (s.Fg >= 0) sb.Append($"\x1b[38;5;{s.Fg}m");
        if (s.Bg >= 0) sb.Append($"\x1b[48;5;{s.Bg}m");
        if (s.Bold) sb.Append("\x1b[1m");
        if (s.Reverse) sb.Append("\x1b[7m");
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
        var sb = new StringBuilder();
        if (lastBg != fill.Bg)
        {
            // Background changed (mode toggle): repaint the whole terminal, not just our buffer.
            AppendSgr(sb, fill);
            sb.Append("\x1b[2J");
            lastBg = fill.Bg;
        }
        sb.Append("\x1b[H");
        Style? cur = null;
        for (int y = 0; y < h; y++)
        {
            sb.Append($"\x1b[{y + 1};1H");
            for (int x = 0; x < w; x++)
            {
                var s = st[y, x];
                if (cur != s)
                {
                    AppendSgr(sb, s);
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
