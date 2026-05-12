# MEX

A very personalized media browser.

## Expected folder and filename layout

<target>/
  <yyyy>/
    yyyy-MM-<slug>-####-<caption>.<ext>     ← slug + caption (preferred — maximum context)
    yyyy-MM-<slug>-####.<ext>               ← slug only
    yyyy-MM-DD-<caption>.<ext>              ← no slug, caption only (counter appended on collision)
    yyyy-MM-DD-####.<ext>                   ← no slug, no caption (counter always — last resort)
