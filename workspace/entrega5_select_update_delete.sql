-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 5 · SELECT, WHERE, UPDATE y DELETE
-- ▶ EJECUTAR → RESULTADOS. Se puede repetir: empieza con su base limpia.
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS entrega5;
CREATE DATABASE entrega5;
USE entrega5;

CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, edad INT,
                          promedio FLOAT, activo BOOLEAN, ingreso DATE);
INSERT INTO estudiantes VALUES
    (1, 'Ana',   20,   88.5, TRUE,  '2023-01-15'),
    (2, 'Luis',  17,   75.0, TRUE,  '2024-01-10'),
    (3, 'Marta', NULL, 90.0, FALSE, '2022-07-01'),
    (4, 'Pedro', 19,   NULL, TRUE,  '2023-08-20'),
    (5, 'Sofía', 16,   95.5, TRUE,  '2025-01-08');

-- El ejemplo del enunciado.
SELECT nombre
FROM estudiantes
WHERE edad >= 18;

-- AND / OR / NOT con paréntesis. Marta (edad NULL) nunca entra en una
-- comparación: NULL >= 18 no es verdadero ni falso.
SELECT nombre, edad FROM estudiantes WHERE activo = TRUE AND (edad < 18 OR promedio > 85);
SELECT nombre FROM estudiantes WHERE NOT (edad >= 18);
SELECT nombre FROM estudiantes WHERE edad IS NULL OR promedio IS NULL;

-- IN, LIKE, BETWEEN y fechas comparadas con cadenas 'YYYY-MM-DD'.
SELECT nombre FROM estudiantes WHERE edad IN (16, 17) OR nombre LIKE 'M%';
SELECT nombre, ingreso FROM estudiantes WHERE ingreso BETWEEN '2023-01-01' AND '2023-12-31';

-- ORDER BY con varias claves, ASC/DESC y alias. Los NULL van al final.
SELECT nombre, edad FROM estudiantes ORDER BY edad DESC;
SELECT nombre, promedio * 10 AS puntos FROM estudiantes ORDER BY activo, puntos DESC;

-- DISTINCT y LIMIT.
SELECT DISTINCT activo FROM estudiantes ORDER BY activo;
SELECT nombre FROM estudiantes ORDER BY promedio DESC LIMIT 2;

-- UPDATE: expresiones, varias columnas a la vez y WHERE.
UPDATE estudiantes SET edad = edad + 1 WHERE edad < 18;
UPDATE estudiantes SET promedio = 0, activo = FALSE WHERE promedio IS NULL;
SELECT nombre, edad, promedio, activo FROM estudiantes ORDER BY id;

-- DELETE con WHERE.
DELETE FROM estudiantes WHERE activo = FALSE;
SELECT nombre FROM estudiantes ORDER BY nombre;
