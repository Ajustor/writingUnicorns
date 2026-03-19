<#
.SYNOPSIS
    Convert assets/icon.png to assets/icon.ico using built-in Windows APIs.
    No external tools required.

.DESCRIPTION
    Generates a multi-size ICO file (256x256, 128x128, 64x64, 48x48, 32x32, 16x16)
    from the PNG source image. The resulting icon.ico is used by the WiX installer
    and can be used anywhere you need an ICO file.

.EXAMPLE
    .\scripts\New-Icon.ps1
    .\scripts\New-Icon.ps1 -Source "assets\icon.png" -Destination "assets\icon.ico"
#>

param(
    [string]$Source      = "$PSScriptRoot\..\assets\icon.png",
    [string]$Destination = "$PSScriptRoot\..\assets\icon.ico"
)

Add-Type -AssemblyName System.Drawing

$Source      = (Resolve-Path $Source).Path
$Destination = [System.IO.Path]::GetFullPath($Destination)

Write-Host "Source:      $Source"
Write-Host "Destination: $Destination"

# Sizes to embed in the ICO (largest first)
$sizes = @(256, 128, 64, 48, 32, 16)

# Load source PNG
$srcBitmap = [System.Drawing.Bitmap]::FromFile($Source)

# ICO binary format:
#   6 bytes  ICONDIR header
#   16 bytes per image  ICONDIRENTRY
#   image data (PNG chunks, stored as-is for 256px; BMP for smaller)

$imageDataList = [System.Collections.Generic.List[byte[]]]::new()

foreach ($size in $sizes) {
    $resized = [System.Drawing.Bitmap]::new($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($resized)
    $g.InterpolationMode  = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode      = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode    = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($srcBitmap, 0, 0, $size, $size)
    $g.Dispose()

    # Save as PNG into a memory stream (ICO supports embedded PNG for >=256px,
    # and most modern renderers support it for smaller sizes too)
    $ms = [System.IO.MemoryStream]::new()
    $resized.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $imageDataList.Add($ms.ToArray())
    $ms.Dispose()
    $resized.Dispose()
}

$srcBitmap.Dispose()

# Write ICO file
$fs = [System.IO.FileStream]::new($Destination, [System.IO.FileMode]::Create)
$bw = [System.IO.BinaryWriter]::new($fs)

$count = $sizes.Count

# ICONDIR
$bw.Write([uint16]0)       # reserved
$bw.Write([uint16]1)       # type: 1 = icon
$bw.Write([uint16]$count)

# Calculate data offset: 6 + 16*count
$dataOffset = 6 + 16 * $count

# ICONDIRENTRY array
for ($i = 0; $i -lt $count; $i++) {
    $sz   = $sizes[$i]
    $data = $imageDataList[$i]

    $bw.Write([byte]($sz -band 0xFF))   # width  (0 = 256)
    $bw.Write([byte]($sz -band 0xFF))   # height (0 = 256)
    $bw.Write([byte]0)                  # color count (0 = true color)
    $bw.Write([byte]0)                  # reserved
    $bw.Write([uint16]1)                # color planes
    $bw.Write([uint16]32)               # bits per pixel
    $bw.Write([uint32]$data.Length)     # size of image data
    $bw.Write([uint32]$dataOffset)      # offset of image data

    $dataOffset += $data.Length
}

# Image data
foreach ($data in $imageDataList) {
    $bw.Write($data)
}

$bw.Close()
$fs.Close()

Write-Host "Done — icon written to: $Destination"
