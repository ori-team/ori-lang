param(
    [int]$Size = 2000,
    [int]$Repeats = 3,
    [switch]$Quick
)

$ErrorActionPreference = "Stop"

if ($Quick) {
    $Size = 200
    $Repeats = 1
}

$Root = Split-Path -Parent $PSScriptRoot
$BenchDir = Join-Path $Root "target\collection-benches"
$Culture = [System.Globalization.CultureInfo]::InvariantCulture
New-Item -ItemType Directory -Force -Path $BenchDir | Out-Null

Push-Location $Root
try {
    cargo build --manifest-path (Join-Path $RepoRoot "compiler/Cargo.toml") -q -p ori-runtime
    & (Join-Path $Root "tools\stage_native_runtime.ps1") | Out-Null

    $cases = @(
        @{
            Name = "list_push_pop"
            Source = @"
namespace bench.list_push_pop

import ori.io as io
import ori.list as lists

func main()
    const values: list<int> = []
    var i: int = 0
    while i < $Size
        lists.push(values, i)
        i = i + 1
    end
    var total: int = 0
    while lists.len(values) > 0
        total = total + lists.pop(values)
    end
    io.print(string(total))
end
"@
        },
        @{
            Name = "map_set_get"
            Source = @"
namespace bench.map_set_get

import ori.io as io
import ori.map as maps

func main()
    const values: map<int, int> = maps.new()
    var i: int = 0
    while i < $Size
        maps.set(values, i, i + 1)
        i = i + 1
    end
    var total: int = 0
    i = 0
    while i < $Size
        total = total + maps.get(values, i)
        i = i + 1
    end
    io.print(string(total))
end
"@
        },
        @{
            Name = "heap_push_pop"
            Source = @"
namespace bench.heap_push_pop

import ori.heap as heap
import ori.io as io

func main()
    const values: heap.Heap<int> = heap.new()
    var i: int = $Size
    while i > 0
        heap.push(values, i)
        i = i - 1
    end
    var total: int = 0
    while heap.len(values) > 0
        match heap.pop(values)
            case some(value):
                total = total + value
            case none:
                total = total + 0
        end
    end
    io.print(string(total))
end
"@
        },
        @{
            Name = "graph_weighted_path"
            Source = @"
namespace bench.graph_weighted_path

import ori.graph as graph
import ori.io as io
import ori.list as lists

func main()
    const values: graph.Graph<int> = graph.new(true)
    var i: int = 0
    while i < $Size
        graph.add_weighted_edge(values, i, i + 1, 1)
        i = i + 1
    end
    match graph.shortest_weighted_path(values, 0, $Size)
        case some(path):
            io.print(string(lists.len(path)))
        case none:
            io.print("missing")
    end
end
"@
        },
        @{
            Name = "linked_cursor_ops"
            Source = @"
namespace bench.linked_cursor_ops

import ori.doubly_linked_list as dll
import ori.io as io

func main()
    const values: dll.DoublyLinkedList<int> = dll.new()
    var i: int = 0
    while i < $Size
        dll.push_back(values, i)
        i = i + 1
    end
    match dll.find(values, $Size / 2)
        case some(cursor):
            io.print(string(dll.insert_after(values, cursor, $Size)))
        case none:
            io.print("missing")
    end
end
"@
        }
    )

    Write-Host "case,size,repeats,min_ms,avg_ms,max_ms"
    foreach ($case in $cases) {
        $sourcePath = Join-Path $BenchDir "$($case.Name).orl"
        $exePath = Join-Path $BenchDir "$($case.Name).exe"
        Set-Content -Path $sourcePath -Value $case.Source -Encoding UTF8
        cargo run --manifest-path (Join-Path $RepoRoot "compiler/Cargo.toml") -q -p ori-driver --bin ori -- compile $sourcePath --out $exePath --native-raw | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "compile failed for $($case.Name)"
        }

        $times = @()
        for ($i = 0; $i -lt $Repeats; $i++) {
            $elapsed = Measure-Command {
                & $exePath | Out-Null
            }
            $times += $elapsed.TotalMilliseconds
        }
        $min = ($times | Measure-Object -Minimum).Minimum
        $avg = ($times | Measure-Object -Average).Average
        $max = ($times | Measure-Object -Maximum).Maximum
        Write-Host ([string]::Format($Culture, "{0},{1},{2},{3:F3},{4:F3},{5:F3}", $case.Name, $Size, $Repeats, $min, $avg, $max))
    }
}
finally {
    Pop-Location
}
