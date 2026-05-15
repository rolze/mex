# MEX

My very personalized media importer and browser. It fits my mental model, most likely it won't fit yours. In that case, fork it and go away. ;)

## Expected folder and filename layout

```
<yyyy>/  yyyy-MM-<slug>-####-<caption>.<ext>   ← slug + caption
         yyyy-MM-<slug>-####.<ext>              ← slug only
         yyyy-MM-DD-<caption>.<ext>             ← caption only
         yyyy-MM-DD-####.<ext>                  ← counter only
```

# Key ideas

* organized by <yyyy> on folder level
* files always prefixed by <yyyy>-<mm>-... 
* <slug> is the only filebased grouper (derived from various hints if available), if not possible it falls back to <yyyy>-<mm>-<dd>-...
* <caption> is an additional potentially valueable information, derived from source filenames, can be extended/changed afterwards by the user to add a short description to the file
* import media files from various devices (mobile phones, camera, etc.) with different filename conventions
* on demand "view" creation based on filter results for album creation and viewing (Linux hard links)
* no mouse support, focus is on fast keyboard interactions
* sophisticated selection and tagging behaviour