package com.clipsync.shizuku;

interface IClipUserService {
    void destroy() = 16777114;
    String getClipboardText() = 1;
    void setClipboardText(String text) = 2;
    int getClipboardHash() = 3;
    String getClipboardMime() = 4;
}
