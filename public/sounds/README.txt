Sons optionnels pour le HUD dictée et la pilule transcription fichier
=======================================================================

Placez ici deux fichiers (mêmes noms) :

  recording-start.mp3   — joué au premier appui sur le raccourci (début d’enregistrement)
  transcribe-start.mp3 — joué au second appui (début de la transcription Whisper)

Pour la transcription de fichier :
  - recording-start.mp3 est joué au début du traitement (phase decode)
  - transcribe-start.mp3 est joué au début de la phase transcription
  - transcribe-start.mp3 est rejoué quand la phase done est reçue

Formats recommandés : MP3, OGG ou WAV (selon ce que le WebView accepte sur votre OS ; MP3 est en général le plus sûr sous Windows).

Si les fichiers sont absents, l’application fonctionne normalement, sans son.

Les fichiers sont copiés dans dist/sounds/ au build. Ne renommez pas les fichiers ci-dessus.
