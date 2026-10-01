# 图标来源与许可

本目录的 27 个 `.svg` 取自 [Tabler Icons](https://tabler.io/icons)（npm 包 `@tabler/icons` v3.48.0，
`icons/outline/` 线性系列），并按本项目需要做了裁剪：删掉源文件里 `stroke="none"` 的占位 path，
去掉 `width/height/class` 属性（尺寸改由 `.slint` 的 `Image` 决定），保留 `stroke="currentColor"`
以便 Slint 的 `colorize` 染色。文件名与语义对应（如 `calculator.svg` = 标准模式），与上游同名文件
的对应关系记录在 `README.md` 的 3.3 节。

上游采用 MIT 许可，全文如下：

---

MIT License

Copyright (c) 2020-2026 Paweł Kuna

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
