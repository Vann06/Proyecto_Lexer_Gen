-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 1 · Lexer y parser
-- ▶ ANALIZAR → pestañas TOKENS y ÁRBOL SINTÁCTICO. Cero problemas.
-- Los errores léxicos y sintácticos están en entrega1_errores.sql.
-- ═══════════════════════════════════════════════════════════════════════

-- Comentario de línea: el lexer lo descarta (no aparece en TOKENS).
/* Comentario de bloque,
   también descartado. */

-- Las palabras clave no distinguen mayúsculas: SELECT = select = SeLeCt.
DROP DATABASE IF EXISTS entrega1;
create database entrega1;
Use entrega1;

-- Tipos de dato, restricciones de columna y de tabla.
CREATE TABLE estudiantes (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(40) NOT NULL,
    edad     INT,
    promedio FLOAT,
    activo   BOOLEAN,
    ingreso  DATE,
    CHECK (edad >= 0)
);

-- Literales: enteros, decimales (con exponente), cadenas con comilla
-- escapada (''), booleanos y NULL. Identificador entre comillas dobles.
INSERT INTO estudiantes (id, nombre, edad, promedio, activo, ingreso) VALUES
    (1, 'Ana',      20, 88.5,   TRUE,  '2023-01-15'),
    (2, 'O''Brien', 17, 7.5e1,  FALSE, '2024-02-29'),
    (3, 'Sofía',    NULL, .5,   NULL,  NULL);

-- Operadores: aritméticos, comparación (=, <>, !=, <, <=, >, >=),
-- lógicos con precedencia NOT > AND > OR, IS NULL, IN, LIKE, BETWEEN.
SELECT nombre, edad * 2 + 1 AS doble FROM estudiantes
WHERE NOT activo = FALSE AND edad <> 0 OR edad IS NULL;

SELECT "nombre" FROM estudiantes WHERE edad IN (17, 20) AND nombre LIKE 'A%' AND promedio BETWEEN 0 AND 100;

-- Todas las sentencias que reconoce la gramática del intérprete.
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30));
CREATE INDEX idx_edad ON estudiantes (edad);
UPDATE estudiantes SET edad = edad + 1 WHERE id = 2;
DELETE FROM estudiantes WHERE id = 3;
SELECT DISTINCT e.nombre FROM estudiantes e INNER JOIN cursos c ON c.id = e.id ORDER BY e.nombre DESC LIMIT 5;
DROP INDEX idx_edad;
DROP TABLE cursos;
SHOW TABLES;
DESCRIBE estudiantes;
